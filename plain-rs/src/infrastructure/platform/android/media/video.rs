use super::audio::{EncodedTrack, write_audio_until};
use super::*;

fn avc_encoder(input: &Format) -> Result<Codec> {
    let width = input
        .int(c"width")
        .ok_or_else(|| anyhow!("Missing video width"))?;
    let height = input
        .int(c"height")
        .ok_or_else(|| anyhow!("Missing video height"))?;
    ensure!(width > 0 && height > 0, "Invalid video dimensions");
    let rate = input
        .int(c"frame-rate")
        .filter(|rate| *rate > 0)
        .unwrap_or(30);
    let bitrate = (i128::from(width) * i128::from(height) * i128::from(rate) * 3 / 20)
        .clamp(4_000_000, 50_000_000) as i32;
    let configure = |high: bool| -> Result<Codec> {
        let format = Format::owned(unsafe { ffi::AMediaFormat_new() })?;
        format.set_string(c"mime", c"video/avc");
        format.set_int(c"width", width);
        format.set_int(c"height", height);
        format.set_int(c"bitrate", bitrate);
        format.set_int(c"frame-rate", rate);
        format.set_int(c"i-frame-interval", 1);
        format.set_int(c"latency", 1);
        format.set_int(c"color-format", 0x7f000789);
        if high {
            format.set_int(c"profile", 8);
        }
        let codec = Codec::create(c"video/avc", true)?;
        codec.configure(&format, ptr::null_mut(), true)?;
        Ok(codec)
    };
    configure(true).or_else(|_| configure(false))
}

pub(super) fn transcode(
    input: &Input,
    output: &Path,
    cancel: &std::sync::atomic::AtomicBool,
) -> Result<bool> {
    transcode_with_audio(input, output, None, cancel)
}

pub(super) fn transcode_with_audio(
    input: &Input,
    output: &Path,
    mut replacement: Option<EncodedTrack>,
    cancel: &std::sync::atomic::AtomicBool,
) -> Result<bool> {
    let extractor = Extractor::open(input)?;
    let formats = extractor.formats()?;
    let video = formats
        .iter()
        .find(|(_, format)| {
            format
                .string(c"mime")
                .is_some_and(|mime| mime.starts_with("video/"))
        })
        .ok_or_else(|| anyhow!("No video track"))?;
    let audio = formats.iter().find(|(_, format)| {
        format
            .string(c"mime")
            .is_some_and(|mime| mime.starts_with("audio/"))
    });
    extractor.select(video.0)?;
    if replacement.is_none() {
        if let Some((index, _)) = audio {
            extractor.select(*index)?;
        }
    }
    let mut surface = Window(ptr::null_mut());
    let mut encoder = avc_encoder(&video.1)?;
    status(
        unsafe { ffi::AMediaCodec_createInputSurface(encoder.raw.as_ptr(), &mut surface.0) },
        "create encoder surface",
    )?;
    ensure!(!surface.0.is_null(), "Missing encoder surface");
    encoder.start()?;
    let mime = CString::new(
        video
            .1
            .string(c"mime")
            .ok_or_else(|| anyhow!("Missing video mime"))?,
    )?;
    let mut decoder = Codec::create(&mime, false)?;
    decoder.configure(&video.1, surface.0, false)?;
    decoder.start()?;
    let mut muxer = Muxer::open(output)?;
    muxer.rotation(video.1.int(c"rotation-degrees").unwrap_or(0))?;
    let audio_out = if let Some(track) = &replacement {
        Some(muxer.add(&track.format)?)
    } else {
        audio.map(|(_, format)| muxer.add(format)).transpose()?
    };
    let mut video_out = None;
    let mut pending = std::collections::VecDeque::<Sample>::new();
    let mut video_eos = false;
    let mut decoder_eos = false;
    let mut encoder_eos = false;
    let mut bytes = Vec::new();
    while !encoder_eos {
        ensure!(
            !cancel.load(std::sync::atomic::Ordering::Acquire),
            "Video conversion cancelled"
        );
        let mut progress = false;
        while !video_eos {
            let index = extractor.track();
            if index < 0 {
                let Some(slot) = decoder.input()? else { break };
                decoder.queue(slot, &[], 0, 4)?;
                video_eos = true;
                progress = true;
                break;
            }
            if index as usize == video.0 {
                let Some(slot) = decoder.input()? else { break };
                let info = extractor.sample(&mut bytes)?;
                decoder.queue(slot, &bytes, info.presentation_time_us, info.flags)?;
            } else if audio.is_some_and(|(track, _)| *track == index as usize) {
                let info = extractor.sample(&mut bytes)?;
                if muxer.started {
                    muxer.write(
                        audio_out.ok_or_else(|| anyhow!("Missing audio output track"))?,
                        &bytes,
                        info,
                    )?;
                } else {
                    pending.push_back(Sample {
                        bytes: bytes.clone(),
                        info,
                    });
                }
            } else {
                anyhow::bail!("Unexpected selected media track")
            }
            extractor.advance();
            progress = true;
        }
        while !decoder_eos {
            let Some((index, info)) = decoder.output(0)? else {
                break;
            };
            progress = true;
            if index >= 0 {
                decoder.release(index as usize, info.size > 0)?;
                if info.flags & 4 != 0 {
                    decoder_eos = true;
                    status(
                        unsafe { ffi::AMediaCodec_signalEndOfInputStream(encoder.raw.as_ptr()) },
                        "signal encoder end",
                    )?;
                }
            }
        }
        let mut timeout = if progress { 0 } else { 10_000 };
        while !encoder_eos {
            ensure!(
                !cancel.load(std::sync::atomic::Ordering::Acquire),
                "Video conversion cancelled"
            );
            let Some((index, info)) = encoder.output(timeout)? else {
                break;
            };
            timeout = 0;
            if index == -2 {
                ensure!(video_out.is_none(), "Encoder output format changed twice");
                video_out = Some(muxer.add(&encoder.format()?)?);
                muxer.start()?;
                while let Some(sample) = pending.pop_front() {
                    muxer.write(
                        audio_out.ok_or_else(|| anyhow!("Missing audio output track"))?,
                        &sample.bytes,
                        sample.info,
                    )?;
                }
            } else if index >= 0 {
                let track = video_out
                    .ok_or_else(|| anyhow!("Encoded samples arrived before the output format"))?;
                if info.size > 0 && info.flags & 2 == 0 {
                    if let Some(audio) = &mut replacement {
                        write_audio_until(
                            &muxer,
                            audio_out.ok_or_else(|| anyhow!("Missing audio output"))?,
                            &mut audio.samples,
                            info.presentation_time_us,
                        )?;
                    }
                    muxer.write(track, encoder.bytes(index as usize)?, info)?;
                }
                encoder.release(index as usize, false)?;
                encoder_eos = info.flags & 4 != 0;
            }
        }
    }
    ensure!(muxer.started, "Encoder produced no media format");
    if let Some(audio) = &mut replacement {
        write_audio_until(
            &muxer,
            audio_out.ok_or_else(|| anyhow!("Missing audio output"))?,
            &mut audio.samples,
            i64::MAX,
        )?;
    }
    muxer.finish()?;
    Ok(true)
}
