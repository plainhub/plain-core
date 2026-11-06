use anyhow::{Context, Result, ensure};
use std::{
    os::windows::ffi::OsStrExt,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};
use windows::{
    Win32::{
        Media::MediaFoundation::*,
        System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize},
    },
    core::{GUID, Interface, PCWSTR},
};

struct Session;

impl Drop for Session {
    fn drop(&mut self) {
        // SAFETY: Interfaces are released before the worker's matching MF/COM shutdown.
        unsafe {
            if let Err(error) = MFShutdown() {
                log::warn!("Media Foundation shutdown: {error}");
            }
            CoUninitialize();
        }
    }
}

struct Stream {
    source: u32,
    destination: u32,
    finished: bool,
}

pub fn transcode(source: &Path, destination: &Path, cancelled: &AtomicBool) -> Result<bool> {
    // SAFETY: COM objects, terminated paths and sample pointers remain on this worker.
    unsafe {
        ensure!(
            !cancelled.load(Ordering::Acquire),
            "Video conversion cancelled"
        );
        CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;
        if let Err(error) = MFStartup(MF_VERSION, MFSTARTUP_FULL) {
            CoUninitialize();
            return Err(error.into());
        }
        let _session = Session;
        let mut attributes = None;
        MFCreateAttributes(&mut attributes, 1)?;
        let attributes = attributes.context("Media attributes missing")?;
        attributes.SetUINT32(&MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, 1)?;
        let source: Vec<u16> = source.as_os_str().encode_wide().chain([0]).collect();
        let destination: Vec<u16> = destination.as_os_str().encode_wide().chain([0]).collect();
        let reader = MFCreateSourceReaderFromURL(PCWSTR(source.as_ptr()), &attributes)?;
        reader.SetStreamSelection(MF_SOURCE_READER_ALL_STREAMS.0 as u32, false)?;
        let mut media_source = std::ptr::null_mut();
        reader.GetServiceForStream(
            MF_SOURCE_READER_MEDIASOURCE.0 as u32,
            &GUID::zeroed(),
            &IMFMediaSource::IID,
            &mut media_source,
        )?;
        let media_source = IMFMediaSource::from_raw(media_source);
        let descriptor = media_source.CreatePresentationDescriptor()?;
        let mut video = None;
        let mut audio = Vec::new();
        for index in 0..descriptor.GetStreamDescriptorCount()? {
            let format = reader.GetNativeMediaType(index, 0)?;
            let kind = format.GetGUID(&MF_MT_MAJOR_TYPE)?;
            if kind == MFMediaType_Video && video.is_none() {
                video = Some((index, format));
            } else if kind == MFMediaType_Audio {
                audio.push((index, format));
            }
        }
        let (video_index, video) = video.context("No video track")?;
        let size = video.GetUINT64(&MF_MT_FRAME_SIZE)?;
        let rate = video.GetUINT64(&MF_MT_FRAME_RATE)?;
        let (width, height) = ((size >> 32) as u32, size as u32);
        let (numerator, denominator) = ((rate >> 32) as u32, rate as u32);
        ensure!(
            width > 0 && height > 0 && numerator > 0 && denominator > 0,
            "Invalid video dimensions or frame rate"
        );
        let output = MFCreateMediaType()?;
        output.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
        output.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_H264)?;
        output.SetUINT64(&MF_MT_FRAME_SIZE, size)?;
        output.SetUINT64(&MF_MT_FRAME_RATE, rate)?;
        output.SetUINT32(
            &MF_MT_AVG_BITRATE,
            plain_libs::media::encoding::h264_bit_rate(
                width,
                height,
                numerator.div_ceil(denominator),
            ),
        )?;
        output.SetUINT32(
            &MF_MT_INTERLACE_MODE,
            video.GetUINT32(&MF_MT_INTERLACE_MODE)?,
        )?;
        match video.GetUINT64(&MF_MT_PIXEL_ASPECT_RATIO) {
            Ok(value) => output.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, value)?,
            Err(error) if error.code() == MF_E_ATTRIBUTENOTFOUND => {}
            Err(error) => return Err(error.into()),
        }
        match video.GetUINT32(&MF_MT_VIDEO_ROTATION) {
            Ok(value) => output.SetUINT32(&MF_MT_VIDEO_ROTATION, value)?,
            Err(error) if error.code() == MF_E_ATTRIBUTENOTFOUND => {}
            Err(error) => return Err(error.into()),
        }
        let decoded = MFCreateMediaType()?;
        decoded.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
        decoded.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_NV12)?;
        reader.SetCurrentMediaType(video_index, None, &decoded)?;
        reader.SetStreamSelection(video_index, true)?;
        let writer = MFCreateSinkWriterFromURL(
            PCWSTR(destination.as_ptr()),
            None::<&IMFByteStream>,
            &attributes,
        )?;
        let stream = writer.AddStream(&output)?;
        writer.SetInputMediaType(
            stream,
            &reader.GetCurrentMediaType(video_index)?,
            None::<&IMFAttributes>,
        )?;
        let mut streams = vec![Stream {
            source: video_index,
            destination: stream,
            finished: false,
        }];
        for (index, format) in audio {
            reader.SetCurrentMediaType(index, None, &format)?;
            reader.SetStreamSelection(index, true)?;
            let stream = writer.AddStream(&format)?;
            writer.SetInputMediaType(stream, &format, None::<&IMFAttributes>)?;
            streams.push(Stream {
                source: index,
                destination: stream,
                finished: false,
            });
        }
        writer.BeginWriting()?;
        while streams.iter().any(|stream| !stream.finished) {
            ensure!(
                !cancelled.load(Ordering::Acquire),
                "Video conversion cancelled"
            );
            let (mut index, mut flags, mut timestamp) = (0, 0, 0);
            let mut sample = None;
            reader.ReadSample(
                MF_SOURCE_READER_ANY_STREAM.0 as u32,
                0,
                Some(&mut index),
                Some(&mut flags),
                Some(&mut timestamp),
                Some(&mut sample),
            )?;
            ensure!(
                flags & MF_SOURCE_READERF_ERROR.0 as u32 == 0,
                "Media Foundation source failed"
            );
            ensure!(
                flags & MF_SOURCE_READERF_CURRENTMEDIATYPECHANGED.0 as u32 == 0,
                "Video source format changed during conversion"
            );
            let stream = streams
                .iter_mut()
                .find(|stream| stream.source == index)
                .context("Unexpected media stream")?;
            if flags & MF_SOURCE_READERF_STREAMTICK.0 as u32 != 0 {
                writer.SendStreamTick(stream.destination, timestamp)?;
            }
            if let Some(sample) = sample {
                writer.WriteSample(stream.destination, &sample)?;
            }
            if flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
                stream.finished = true;
            }
        }
        ensure!(
            !cancelled.load(Ordering::Acquire),
            "Video conversion cancelled"
        );
        writer.Finalize()?;
        Ok(true)
    }
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/infrastructure/platform/windows/video.rs"]
mod tests;
