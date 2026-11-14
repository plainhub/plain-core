use plain_libs::anyhow::{Context, Result, anyhow, ensure};
use block2::RcBlock;
use dispatch2::{DispatchQueue, DispatchQueueAttr};
use objc2::{
    AnyThread,
    rc::{Retained, autoreleasepool},
    runtime::AnyObject,
};
use objc2_av_foundation::*;
use objc2_core_media::{CMSampleBuffer, kCMTimeZero};
use objc2_core_video::{
    kCVPixelBufferPixelFormatTypeKey, kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange,
};
use objc2_foundation::{NSDictionary, NSNumber, NSString, NSURL};
use std::{
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::Duration,
};

#[cfg(test)]
std::thread_local! {
    static READY_REQUESTS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static NORMALIZED_SOURCES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

struct Session {
    reader: Retained<AVAssetReader>,
    writer: Retained<AVAssetWriter>,
}

impl Drop for Session {
    fn drop(&mut self) {
        // SAFETY: The session is created, used and destroyed on one media worker.
        unsafe {
            if self.reader.status() == AVAssetReaderStatus::Reading {
                self.reader.cancelReading();
            }
            if self.writer.status() == AVAssetWriterStatus::Writing {
                self.writer.cancelWriting();
            }
        }
    }
}

struct Stream {
    output: Retained<AVAssetReaderTrackOutput>,
    input: Retained<AVAssetWriterInput>,
    pending: Option<Retained<CMSampleBuffer>>,
    finished: bool,
}

pub fn transcode(source: &Path, destination: &Path, canceled: &AtomicBool) -> Result<bool> {
    ensure!(
        !canceled.load(Ordering::Acquire),
        "Video conversion canceled"
    );
    let normalized = if plain_libs::media::isobmff::tracks(source)
        .and_then(|tracks| tracks.first_video_codec)
        .as_deref()
        == Some("hev1")
    {
        let mut temporary = plain_libs::temporary::Builder::new().suffix(".mp4").tempfile_in(
            destination
                .parent()
                .context("Missing video output directory")?,
        )?;
        plain_libs::media::isobmff::canonicalize_hev1(
            &mut std::fs::File::open(source)?,
            temporary.as_file_mut(),
            canceled,
        )?;
        #[cfg(test)]
        NORMALIZED_SOURCES.set(NORMALIZED_SOURCES.get() + 1);
        Some(temporary)
    } else {
        None
    };
    transcode_file(
        normalized.as_ref().map_or(source, |file| file.path()),
        destination,
        canceled,
    )
}

fn transcode_file(source: &Path, destination: &Path, canceled: &AtomicBool) -> Result<bool> {
    autoreleasepool(|_| {
        // SAFETY: Objects stay on this worker and settings use AVFoundation's documented types.
        unsafe {
            ensure!(
                !canceled.load(Ordering::Acquire),
                "Video conversion canceled"
            );
            let video_type =
                AVMediaTypeVideo.context("AVFoundation video media type unavailable")?;
            let audio_type =
                AVMediaTypeAudio.context("AVFoundation audio media type unavailable")?;
            let asset = AVAsset::assetWithURL(&NSURL::fileURLWithPath(&NSString::from_str(
                source.to_str().context("Invalid video path")?,
            )));
            let video = asset
                .tracksWithMediaType(video_type)
                .firstObject()
                .context("No video track")?;
            let reader = AVAssetReader::assetReaderWithAsset_error(&asset)
                .map_err(|e| anyhow!(e.to_string()))?;
            let size = video.naturalSize();
            ensure!(
                size.width.is_finite()
                    && size.height.is_finite()
                    && size.width > 0.0
                    && size.height > 0.0,
                "Invalid video dimensions"
            );
            let pixel_key = NSString::from_str(&kCVPixelBufferPixelFormatTypeKey.to_string());
            let pixel_format = NSNumber::new_u32(kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange);
            let pixels: [&AnyObject; 1] = [&pixel_format];
            let decoded = NSDictionary::from_slices(&[&*pixel_key], &pixels);
            let video_output = AVAssetReaderTrackOutput::initWithTrack_outputSettings(
                AVAssetReaderTrackOutput::alloc(),
                &video,
                Some(&decoded),
            );
            video_output.setAlwaysCopiesSampleData(false);
            ensure!(
                reader.canAddOutput(&video_output),
                "Video decoder unavailable"
            );
            reader.addOutput(&video_output);
            let mut audio_outputs = Vec::new();
            let audio_tracks = asset.tracksWithMediaType(audio_type);
            for index in 0..audio_tracks.count() {
                let track = audio_tracks.objectAtIndex(index);
                let output = AVAssetReaderTrackOutput::initWithTrack_outputSettings(
                    AVAssetReaderTrackOutput::alloc(),
                    &track,
                    None,
                );
                output.setAlwaysCopiesSampleData(false);
                ensure!(reader.canAddOutput(&output), "Audio track cannot be read");
                reader.addOutput(&output);
                audio_outputs.push((output, track.extendedLanguageTag()));
            }
            std::fs::remove_file(destination)?;
            let writer = AVAssetWriter::assetWriterWithURL_fileType_error(
                &NSURL::fileURLWithPath(&NSString::from_str(
                    destination.to_str().context("Invalid video output path")?,
                )),
                AVFileTypeMPEG4.context("AVFoundation MP4 writer unavailable")?,
            )
            .map_err(|e| anyhow!(e.to_string()))?;
            let session = Session { reader, writer };
            session.writer.setShouldOptimizeForNetworkUse(true);
            let width = NSNumber::new_f64(size.width);
            let height = NSNumber::new_f64(size.height);
            let values: [&AnyObject; 3] = [
                AVVideoCodecTypeH264.context("H.264 encoder unavailable")?,
                &width,
                &height,
            ];
            let settings = NSDictionary::from_slices(
                &[
                    AVVideoCodecKey.context("Video codec setting unavailable")?,
                    AVVideoWidthKey.context("Video width setting unavailable")?,
                    AVVideoHeightKey.context("Video height setting unavailable")?,
                ],
                &values,
            );
            ensure!(
                session
                    .writer
                    .canApplyOutputSettings_forMediaType(Some(&settings), video_type),
                "H.264 output settings unsupported"
            );
            let input = AVAssetWriterInput::assetWriterInputWithMediaType_outputSettings(
                video_type,
                Some(&settings),
            );
            input.setTransform(video.preferredTransform());
            input.setExtendedLanguageTag(video.extendedLanguageTag().as_deref());
            ensure!(
                session.writer.canAddInput(&input),
                "H.264 output cannot be written"
            );
            session.writer.addInput(&input);
            let mut streams = vec![Stream {
                output: video_output,
                input,
                pending: None,
                finished: false,
            }];
            ensure!(
                session.reader.startReading(),
                "{}",
                session
                    .reader
                    .error()
                    .map(|e| e.to_string())
                    .unwrap_or_else(|| "Video reader did not start".into())
            );
            for (output, language) in audio_outputs {
                if let Some(sample) = output.copyNextSampleBuffer() {
                    let format = sample
                        .format_description()
                        .context("Audio format missing")?;
                    let input = AVAssetWriterInput::assetWriterInputWithMediaType_outputSettings_sourceFormatHint(audio_type, None, Some(&format));
                    ensure!(
                        session.writer.canAddInput(&input),
                        "Audio passthrough unavailable for MP4"
                    );
                    input.setExtendedLanguageTag(language.as_deref());
                    session.writer.addInput(&input);
                    streams.push(Stream {
                        output,
                        input,
                        pending: Some(sample),
                        finished: false,
                    });
                }
            }
            ensure!(
                session.writer.startWriting(),
                "{}",
                session
                    .writer
                    .error()
                    .map(|e| e.to_string())
                    .unwrap_or_else(|| "Video writer did not start".into())
            );
            session.writer.startSessionAtSourceTime(kCMTimeZero);
            let queue = DispatchQueue::new("plain.video.writer", DispatchQueueAttr::SERIAL);
            let (ready_sender, ready_receiver) = mpsc::sync_channel(0);
            // The callback transfers only a wakeup; native objects stay on this worker.
            let ready = RcBlock::new(move || {
                let _ = ready_sender.send(());
            });
            for stream in &streams {
                stream
                    .input
                    .requestMediaDataWhenReadyOnQueue_usingBlock(&queue, &ready);
                #[cfg(test)]
                READY_REQUESTS.set(READY_REQUESTS.get() + 1);
            }
            while streams.iter().any(|stream| !stream.finished) {
                ensure!(
                    !canceled.load(Ordering::Acquire),
                    "Video conversion canceled"
                );
                ensure!(
                    session.writer.status() == AVAssetWriterStatus::Writing,
                    "{}",
                    session
                        .writer
                        .error()
                        .map(|e| e.to_string())
                        .unwrap_or_else(|| "Video writer stopped".into())
                );
                ensure!(
                    matches!(
                        session.reader.status(),
                        AVAssetReaderStatus::Reading | AVAssetReaderStatus::Completed
                    ),
                    "{}",
                    session
                        .reader
                        .error()
                        .map(|e| e.to_string())
                        .unwrap_or_else(|| "Video reader stopped".into())
                );
                let mut progress = false;
                for stream in streams.iter_mut().filter(|stream| !stream.finished) {
                    if !stream.input.isReadyForMoreMediaData() {
                        continue;
                    }
                    progress = true;
                    if let Some(sample) = stream
                        .pending
                        .take()
                        .or_else(|| stream.output.copyNextSampleBuffer())
                    {
                        ensure!(
                            stream.input.appendSampleBuffer(&sample),
                            "{}",
                            session
                                .writer
                                .error()
                                .map(|e| e.to_string())
                                .unwrap_or_else(|| "Video sample rejected".into())
                        );
                    } else {
                        stream.input.markAsFinished();
                        stream.finished = true;
                    }
                }
                if !progress {
                    match ready_receiver.recv_timeout(Duration::from_millis(20)) {
                        Ok(()) | Err(mpsc::RecvTimeoutError::Timeout) => {}
                        Err(error) => return Err(error.into()),
                    }
                }
            }
            drop(ready_receiver);
            ensure!(
                session.reader.status() == AVAssetReaderStatus::Completed,
                "{}",
                session
                    .reader
                    .error()
                    .map(|e| e.to_string())
                    .unwrap_or_else(|| "Video reader did not complete".into())
            );
            let (sender, receiver) = mpsc::channel();
            let finished = RcBlock::new(move || {
                let _ = sender.send(());
            });
            session.writer.finishWritingWithCompletionHandler(&finished);
            while session.writer.status() == AVAssetWriterStatus::Writing {
                ensure!(
                    !canceled.load(Ordering::Acquire),
                    "Video conversion canceled"
                );
                match receiver.recv_timeout(Duration::from_millis(20)) {
                    Ok(()) => break,
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                    Err(error) => return Err(error.into()),
                }
            }
            ensure!(
                session.writer.status() == AVAssetWriterStatus::Completed,
                "{}",
                session
                    .writer
                    .error()
                    .map(|e| e.to_string())
                    .unwrap_or_else(|| "Video writer did not complete".into())
            );
            Ok(true)
        }
    })
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/infrastructure/platform/apple/video.rs"]
mod tests;
