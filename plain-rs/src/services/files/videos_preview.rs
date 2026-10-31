//! Video codec probing and browser-playability transcoding.
//!
//! Mirrors plain-app `probeVideoCodec` / `transcodeMp4ForBrowser`: the shared
//! web client probes `/fs?probe=1` for the first video track's codec fourcc
//! and switches to `/fs?tr=1` on browsers without an HEVC decoder. plain-app
//! runs its probe in-process (Android `MediaExtractor`); the equivalent here
//! is an in-tree ISO-BMFF walker — spawning `ffprobe` cost ~0.5s of CPU on a
//! weak the box per first view, which dominated video start latency. The
//! `ffmpeg` CLI remains the fallback for non-ISO containers (Matroska, WebM)
//! and the transcode path.

use anyhow::Result;

fn probe_iso_bmff_video_fourcc(path: &std::path::Path) -> Option<String> {
    plain_rs::media::isobmff::tracks(path)?.first_video_codec
}

/// fourcc of the first video track's codec tag (e.g. "hvc1", "avc1"), or ""
/// when the file has no parsable video track. Non alphanumeric characters
/// (e.g. `[0][0][0][0]` for untagged streams) are stripped, mirroring
/// plain-app. ISO-BMFF files are parsed in-process (sub-millisecond);
/// everything else falls back to the `ffprobe` CLI.
pub async fn probe_video_codec(path: &str) -> String {
    probe_with_fallback(path, |p: String| async move { ffprobe_codec_tag(&p).await }).await
}

/// `fallback` is only consulted when the in-process walker cannot answer
/// (non-ISO container / unparsable file). It is injected so tests can prove
/// deterministically that ISO-BMFF probes never pay the ~0.5 s process
/// spawn on weak CPUs — the whole point of the walker.
async fn probe_with_fallback<F, Fut>(path: &str, fallback: F) -> String
where
    F: FnOnce(String) -> Fut,
    Fut: std::future::Future<Output = String>,
{
    let owned = path.to_string();
    let in_process = tokio::task::spawn_blocking(move || {
        probe_iso_bmff_video_fourcc(std::path::Path::new(&owned))
    })
    .await;
    if let Ok(Some(fourcc)) = in_process {
        return filter_codec_tag(&fourcc);
    }
    fallback(path.to_string()).await
}

async fn ffprobe_codec_tag(path: &str) -> String {
    let out = tokio::process::Command::new("ffprobe")
        .args([
            "-v",
            "quiet",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=codec_tag_string",
            "-of",
            "csv=p=0",
            path,
        ])
        .output()
        .await;
    match out {
        Ok(out) if out.status.success() => {
            let tag = String::from_utf8_lossy(&out.stdout).trim().to_string();
            filter_codec_tag(&tag)
        }
        _ => String::new(),
    }
}

/// Keep only alphanumeric characters, mirroring plain-app
/// `probeVideoCodec(...).filter { it.isLetterOrDigit() }`.
fn filter_codec_tag(tag: &str) -> String {
    tag.chars().filter(|c| c.is_alphanumeric()).collect()
}

pub async fn encode_browser_video(
    input: &std::path::Path,
    output: &std::path::Path,
    transcode: bool,
) -> Result<bool> {
    let mut command = tokio::process::Command::new("ffmpeg");
    command
        .kill_on_drop(true)
        .args(["-y", "-v", "error", "-i"])
        .arg(input);
    if transcode {
        command.args([
            "-map", "0:v:0", "-map", "0:a?", "-c:v", "libx264", "-preset", "veryfast", "-crf",
            "23", "-pix_fmt", "yuv420p", "-c:a", "copy",
        ]);
    } else {
        command.args(["-map", "0:v?", "-map", "0:a?", "-c", "copy"]);
    }
    let result = command
        .args(["-movflags", "+faststart", "-f", "mp4"])
        .arg(output)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .output()
        .await?;
    anyhow::ensure!(
        result.status.success(),
        "Video encoder failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    Ok(true)
}

pub async fn encode_3gp(input: &std::path::Path, output: &std::path::Path) -> Result<bool> {
    let result = tokio::process::Command::new("ffmpeg")
        .kill_on_drop(true)
        .args(["-y", "-v", "error", "-i"])
        .arg(input)
        .args([
            "-map",
            "0:v:0",
            "-map",
            "0:a?",
            "-c:v",
            "libx264",
            "-c:a",
            "aac",
            "-movflags",
            "+faststart",
            "-f",
            "mp4",
        ])
        .arg(output)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .output()
        .await?;
    anyhow::ensure!(
        result.status.success(),
        "Video encoder failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    Ok(true)
}

#[cfg(test)]
#[path = "../../../tests/unit/services/files/videos_preview.rs"]
mod tests;
