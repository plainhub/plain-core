use super::*;

pub(super) struct EncodedTrack {
    pub(super) format: Format,
    pub(super) samples: std::collections::VecDeque<Sample>,
}
pub(super) fn write_audio_until(
    muxer: &Muxer,
    track: usize,
    samples: &mut std::collections::VecDeque<Sample>,
    pts: i64,
) -> Result<()> {
    while samples
        .front()
        .is_some_and(|sample| sample.info.presentation_time_us <= pts)
    {
        let sample = samples
            .pop_front()
            .ok_or_else(|| anyhow!("Missing audio sample"))?;
        muxer.write(track, &sample.bytes, sample.info)?;
    }
    Ok(())
}
fn audio_aac(
    input: &Input,
    cancel: &std::sync::atomic::AtomicBool,
) -> Result<Option<EncodedTrack>> {
    let extractor = Extractor::open(input)?;
    let formats = extractor.formats()?;
    let Some((index, format)) = formats.iter().find(|(_, format)| {
        format
            .string(c"mime")
            .is_some_and(|mime| mime.starts_with("audio/"))
    }) else {
        return Ok(None);
    };
    extractor.select(*index)?;
    let mime = CString::new(
        format
            .string(c"mime")
            .ok_or_else(|| anyhow!("Missing audio mime"))?,
    )?;
    let mut decoder = Codec::create(&mime, false)?;
    decoder.configure(format, ptr::null_mut(), false)?;
    decoder.start()?;
    let output = Format::owned(unsafe { ffi::AMediaFormat_new() })?;
    output.set_string(c"mime", c"audio/mp4a-latm");
    output.set_int(
        c"sample-rate",
        format
            .int(c"sample-rate")
            .ok_or_else(|| anyhow!("Missing sample rate"))?,
    );
    output.set_int(
        c"channel-count",
        format
            .int(c"channel-count")
            .ok_or_else(|| anyhow!("Missing channel count"))?,
    );
    output.set_int(c"aac-profile", 2);
    output.set_int(c"bitrate", 128_000);
    let mut encoder = Codec::create(c"audio/mp4a-latm", true)?;
    encoder.configure(&output, ptr::null_mut(), true)?;
    encoder.start()?;
    let mut input_eos = false;
    let mut decoder_eos = false;
    let mut encoder_eos = false;
    let mut pcm = std::collections::VecDeque::<Sample>::new();
    let mut samples = std::collections::VecDeque::new();
    let mut result_format = None;
    let mut bytes = Vec::new();
    while !encoder_eos {
        ensure!(
            !cancel.load(std::sync::atomic::Ordering::Acquire),
            "Video conversion cancelled"
        );
        let mut progress = false;
        if pcm.is_empty() {
            while !input_eos {
                let Some(slot) = decoder.input()? else { break };
                if extractor.track() < 0 {
                    decoder.queue(slot, &[], 0, 4)?;
                    input_eos = true;
                } else {
                    let info = extractor.sample(&mut bytes)?;
                    decoder.queue(slot, &bytes, info.presentation_time_us, info.flags)?;
                    extractor.advance();
                }
                progress = true;
            }
        }
        while !decoder_eos {
            let Some((index, mut info)) = decoder.output(0)? else {
                break;
            };
            progress = true;
            if index >= 0 {
                let data = if info.size > 0 {
                    ensure!(info.offset >= 0 && info.size >= 0, "Invalid PCM bounds");
                    let end = (info.offset as usize)
                        .checked_add(info.size as usize)
                        .ok_or_else(|| anyhow!("PCM size overflow"))?;
                    decoder
                        .bytes(index as usize)?
                        .get(info.offset as usize..end)
                        .ok_or_else(|| anyhow!("Invalid PCM bounds"))?
                        .to_vec()
                } else {
                    Vec::new()
                };
                decoder_eos = info.flags & 4 != 0;
                if info.flags & 2 == 0 && (!data.is_empty() || decoder_eos) {
                    info.offset = 0;
                    pcm.push_back(Sample { bytes: data, info });
                }
                decoder.release(index as usize, false)?;
            }
        }
        while !pcm.is_empty() {
            let Some(slot) = encoder.input()? else { break };
            let sample = pcm
                .pop_front()
                .ok_or_else(|| anyhow!("Missing PCM sample"))?;
            encoder.queue(
                slot,
                &sample.bytes,
                sample.info.presentation_time_us,
                sample.info.flags & 4,
            )?;
            progress = true;
        }
        let mut timeout = if progress { 0 } else { 10_000 };
        while !encoder_eos {
            let Some((index, mut info)) = encoder.output(timeout)? else {
                break;
            };
            timeout = 0;
            if index == -2 {
                ensure!(result_format.is_none(), "Audio output format changed twice");
                result_format = Some(encoder.format()?);
            } else if index >= 0 {
                let eos = info.flags & 4 != 0;
                if info.size > 0 && info.flags & 2 == 0 {
                    ensure!(info.offset >= 0, "Invalid audio sample offset");
                    let end = (info.offset as usize)
                        .checked_add(info.size as usize)
                        .ok_or_else(|| anyhow!("Audio size overflow"))?;
                    let data = encoder
                        .bytes(index as usize)?
                        .get(info.offset as usize..end)
                        .ok_or_else(|| anyhow!("Invalid audio sample bounds"))?
                        .to_vec();
                    info.offset = 0;
                    info.flags &= 1;
                    samples.push_back(Sample { bytes: data, info });
                }
                encoder.release(index as usize, false)?;
                encoder_eos = eos;
            }
        }
    }
    Ok(Some(EncodedTrack {
        format: result_format.ok_or_else(|| anyhow!("Audio encoder produced no format"))?,
        samples,
    }))
}

pub(super) fn convert_3gp(
    input: &Input,
    output: &Path,
    cancel: &std::sync::atomic::AtomicBool,
) -> Result<bool> {
    let mut audio = audio_aac(input, cancel)?;
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
    if video.1.string(c"mime").as_deref() != Some("video/avc") {
        return super::video::transcode_with_audio(input, output, audio, cancel);
    }
    extractor.select(video.0)?;
    let mut muxer = Muxer::open(output)?;
    muxer.rotation(video.1.int(c"rotation-degrees").unwrap_or(0))?;
    let video_out = muxer.add(&video.1)?;
    let audio_out = audio
        .as_ref()
        .map(|audio| muxer.add(&audio.format))
        .transpose()?;
    muxer.start()?;
    let mut bytes = Vec::new();
    while extractor.track() >= 0 {
        ensure!(
            !cancel.load(std::sync::atomic::Ordering::Acquire),
            "Video conversion cancelled"
        );
        let info = extractor.sample(&mut bytes)?;
        if let Some(audio) = &mut audio {
            write_audio_until(
                &muxer,
                audio_out.ok_or_else(|| anyhow!("Missing audio output"))?,
                &mut audio.samples,
                info.presentation_time_us,
            )?;
        }
        muxer.write(video_out, &bytes, info)?;
        extractor.advance();
    }
    if let Some(audio) = &mut audio {
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
