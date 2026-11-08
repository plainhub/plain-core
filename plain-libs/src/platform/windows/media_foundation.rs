use crate::platform::rename::wide_path;
use anyhow::{Context, Result, ensure};
use std::{
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

mod bindings;
use bindings::*;

struct Stream {
    source: u32,
    destination: u32,
    finished: bool,
}

pub fn transcode(source: &Path, destination: &Path, cancelled: &AtomicBool) -> Result<bool> {
    ensure!(
        !cancelled.load(Ordering::Acquire),
        "Video conversion cancelled"
    );
    let _session = Session::start()?;
    let attributes = create_attributes()?;
    attributes.set_u32(&MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, 1)?;
    let source = wide_path(source)?;
    let destination = wide_path(destination)?;
    let reader = source_reader(&source, &attributes)?;
    reader.select(ALL_STREAMS, false)?;
    let media_source = reader.media_source()?;
    let descriptor = media_source.descriptor()?;
    let mut video = None;
    let mut audio = Vec::new();
    for index in 0..descriptor.stream_count()? {
        let format = reader.native_media_type(index)?;
        let kind = format.guid(&MF_MT_MAJOR_TYPE)?;
        if kind == MF_MEDIA_TYPE_VIDEO && video.is_none() {
            video = Some((index, format));
        } else if kind == MF_MEDIA_TYPE_AUDIO {
            audio.push((index, format));
        }
    }
    let (video_index, video) = video.context("No video track")?;
    let size = video.u64(&MF_MT_FRAME_SIZE)?;
    let rate = video.u64(&MF_MT_FRAME_RATE)?;
    let (width, height) = ((size >> 32) as u32, size as u32);
    let (numerator, denominator) = ((rate >> 32) as u32, rate as u32);
    ensure!(
        width > 0 && height > 0 && numerator > 0 && denominator > 0,
        "Invalid video dimensions or frame rate"
    );
    let output = create_media_type()?;
    output.set_guid(&MF_MT_MAJOR_TYPE, &MF_MEDIA_TYPE_VIDEO)?;
    output.set_guid(&MF_MT_SUBTYPE, &MF_VIDEO_FORMAT_H264)?;
    output.set_u64(&MF_MT_FRAME_SIZE, size)?;
    output.set_u64(&MF_MT_FRAME_RATE, rate)?;
    output.set_u32(
        &MF_MT_AVG_BITRATE,
        crate::media::encoding::h264_bit_rate(width, height, numerator.div_ceil(denominator)),
    )?;
    output.set_u32(&MF_MT_INTERLACE_MODE, video.u32(&MF_MT_INTERLACE_MODE)?)?;
    match video.u64(&MF_MT_PIXEL_ASPECT_RATIO) {
        Ok(value) => output.set_u64(&MF_MT_PIXEL_ASPECT_RATIO, value)?,
        Err(error) if error == ATTRIBUTE_NOT_FOUND => {}
        Err(error) => return Err(error.into()),
    }
    match video.u32(&MF_MT_VIDEO_ROTATION) {
        Ok(value) => output.set_u32(&MF_MT_VIDEO_ROTATION, value)?,
        Err(error) if error == ATTRIBUTE_NOT_FOUND => {}
        Err(error) => return Err(error.into()),
    }
    let decoded = create_media_type()?;
    decoded.set_guid(&MF_MT_MAJOR_TYPE, &MF_MEDIA_TYPE_VIDEO)?;
    decoded.set_guid(&MF_MT_SUBTYPE, &MF_VIDEO_FORMAT_NV12)?;
    reader.set_media_type(video_index, &decoded)?;
    reader.select(video_index, true)?;
    let writer = sink_writer(&destination, &attributes)?;
    let stream = writer.add_stream(&output)?;
    writer.set_input(stream, &reader.current_media_type(video_index)?)?;
    let mut streams = vec![Stream {
        source: video_index,
        destination: stream,
        finished: false,
    }];
    for (index, format) in audio {
        reader.set_media_type(index, &format)?;
        reader.select(index, true)?;
        let stream = writer.add_stream(&format)?;
        writer.set_input(stream, &format)?;
        streams.push(Stream {
            source: index,
            destination: stream,
            finished: false,
        });
    }
    writer.begin()?;
    while streams.iter().any(|stream| !stream.finished) {
        ensure!(
            !cancelled.load(Ordering::Acquire),
            "Video conversion cancelled"
        );
        let (index, flags, timestamp, sample) = reader.read_sample()?;
        ensure!(flags & SOURCE_ERROR == 0, "Media Foundation source failed");
        ensure!(
            flags & MEDIA_TYPE_CHANGED == 0,
            "Video source format changed during conversion"
        );
        let stream = streams
            .iter_mut()
            .find(|stream| stream.source == index)
            .context("Unexpected media stream")?;
        if flags & STREAM_TICK != 0 {
            writer.send_tick(stream.destination, timestamp)?;
        }
        if let Some(sample) = sample {
            writer.write_sample(stream.destination, &sample)?;
        }
        if flags & END_OF_STREAM != 0 {
            stream.finished = true;
        }
    }
    ensure!(
        !cancelled.load(Ordering::Acquire),
        "Video conversion cancelled"
    );
    writer.finish()?;
    Ok(true)
}
