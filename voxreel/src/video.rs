//! ffmpeg plumbing: raw RGBA frames in on stdin, H.264 + AAC mp4 out.
//!
//! Frames go to ffmpeg from a writer thread, so rendering the next frame overlaps with encoding.
//! On macOS the hardware encoder (VideoToolbox) is used by default: x264 "medium" was the
//! bottleneck at ~25 fps for 1080x1920@60.

use anyhow::{bail, Context, Result};
use std::io::Write;
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::thread::JoinHandle;

pub struct Encoder {
    child: Child,
    tx: Option<SyncSender<Vec<u8>>>,
    writer: Option<JoinHandle<Result<()>>>,
    /// Frame buffers handed back by the writer thread for reuse.
    spare: Receiver<Vec<u8>>,
}

/// "auto" (VideoToolbox on macOS, x264 elsewhere), "videotoolbox" or "x264".
fn codec_args(codec: &str) -> Vec<&'static str> {
    let hw = match codec {
        "x264" => false,
        "videotoolbox" => true,
        _ => cfg!(target_os = "macos"),
    };
    if hw {
        // High bitrate: YouTube re-encodes anyway, so spend bits, not time.
        vec!["-c:v", "h264_videotoolbox", "-profile:v", "high", "-b:v", "28M", "-maxrate", "40M", "-bufsize", "40M"]
    } else {
        vec!["-c:v", "libx264", "-preset", "medium", "-crf", "17"]
    }
}

impl Encoder {
    pub fn start(out: &Path, w: u32, h: u32, fps: u32, audio: Option<&Path>, codec: &str) -> Result<Self> {
        let mut cmd = Command::new("ffmpeg");
        cmd.args(["-hide_banner", "-loglevel", "error", "-y"])
            .args(["-f", "rawvideo", "-pix_fmt", "rgba", "-s", &format!("{w}x{h}"), "-r", &fps.to_string(), "-i", "-"]);
        if let Some(a) = audio {
            cmd.arg("-i").arg(a);
        }
        cmd.args(codec_args(codec)).args(["-pix_fmt", "yuv420p", "-movflags", "+faststart"]);
        if audio.is_some() {
            cmd.args(["-c:a", "aac", "-b:a", "192k", "-shortest"]);
        }
        cmd.arg(out).stdin(Stdio::piped());
        let mut child = cmd.spawn().context("failed to start ffmpeg (is it installed and on PATH?)")?;
        let stdin = child.stdin.take().context("ffmpeg stdin")?;
        Ok(Self::with_writer(child, stdin))
    }

    fn with_writer(child: Child, mut stdin: ChildStdin) -> Self {
        let (tx, rx) = sync_channel::<Vec<u8>>(3);
        let (back_tx, spare) = sync_channel::<Vec<u8>>(4);
        let writer = std::thread::spawn(move || -> Result<()> {
            for frame in rx {
                stdin.write_all(&frame).context("ffmpeg stdin")?;
                let _ = back_tx.try_send(frame);
            }
            Ok(())
        });
        Self { child, tx: Some(tx), writer: Some(writer), spare }
    }

    pub fn frame(&mut self, rgba: &[u8]) -> Result<()> {
        let mut buf = self.spare.try_recv().unwrap_or_default();
        buf.clear();
        buf.extend_from_slice(rgba);
        if self.tx.as_ref().context("encoder closed")?.send(buf).is_err() {
            // The writer stopped (ffmpeg died): surface its error.
            self.join_writer()?;
            bail!("ffmpeg stopped accepting frames");
        }
        Ok(())
    }

    fn join_writer(&mut self) -> Result<()> {
        drop(self.tx.take());
        match self.writer.take() {
            Some(h) => h.join().map_err(|_| anyhow::anyhow!("encoder thread panicked"))?,
            None => Ok(()),
        }
    }

    pub fn finish(mut self) -> Result<()> {
        self.join_writer()?;
        let status = self.child.wait()?;
        if !status.success() {
            bail!("ffmpeg exited with {status}");
        }
        Ok(())
    }
}

pub fn write_png(path: &Path, rgba: &[u8], w: u32, h: u32) -> Result<()> {
    let mut child = Command::new("ffmpeg")
        .args(["-hide_banner", "-loglevel", "error", "-y"])
        .args(["-f", "rawvideo", "-pix_fmt", "rgba", "-s", &format!("{w}x{h}"), "-i", "-", "-frames:v", "1"])
        .arg(path)
        .stdin(Stdio::piped())
        .spawn()
        .context("failed to start ffmpeg")?;
    child.stdin.take().context("ffmpeg stdin")?.write_all(rgba)?;
    let status = child.wait()?;
    if !status.success() {
        bail!("ffmpeg exited with {status}");
    }
    Ok(())
}
