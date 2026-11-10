//! ffmpeg plumbing: raw RGBA frames in on stdin, H.264 + AAC mp4 out.

use anyhow::{bail, Context, Result};
use std::io::Write;
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};

pub struct Encoder {
    child: Child,
    stdin: Option<ChildStdin>,
}

impl Encoder {
    pub fn start(out: &Path, w: u32, h: u32, fps: u32, audio: Option<&Path>) -> Result<Self> {
        let mut cmd = Command::new("ffmpeg");
        cmd.args(["-hide_banner", "-loglevel", "error", "-y"])
            .args(["-f", "rawvideo", "-pix_fmt", "rgba", "-s", &format!("{w}x{h}"), "-r", &fps.to_string(), "-i", "-"]);
        if let Some(a) = audio {
            cmd.arg("-i").arg(a);
        }
        cmd.args(["-c:v", "libx264", "-preset", "medium", "-crf", "17", "-pix_fmt", "yuv420p", "-movflags", "+faststart"]);
        if audio.is_some() {
            cmd.args(["-c:a", "aac", "-b:a", "192k", "-shortest"]);
        }
        cmd.arg(out).stdin(Stdio::piped());
        let mut child = cmd.spawn().context("failed to start ffmpeg (is it installed and on PATH?)")?;
        let stdin = child.stdin.take();
        Ok(Self { child, stdin })
    }

    pub fn frame(&mut self, rgba: &[u8]) -> Result<()> {
        self.stdin.as_mut().context("encoder closed")?.write_all(rgba).context("ffmpeg stdin")
    }

    pub fn finish(mut self) -> Result<()> {
        drop(self.stdin.take());
        let status = self.child.wait()?;
        if !status.success() {
            bail!("ffmpeg exited with {status}");
        }
        Ok(())
    }
}

pub fn write_png(path: &Path, rgba: &[u8], w: u32, h: u32) -> Result<()> {
    let mut enc = Encoder::start_still(path, w, h)?;
    enc.frame(rgba)?;
    enc.finish()
}

impl Encoder {
    fn start_still(out: &Path, w: u32, h: u32) -> Result<Self> {
        let mut child = Command::new("ffmpeg")
            .args(["-hide_banner", "-loglevel", "error", "-y"])
            .args(["-f", "rawvideo", "-pix_fmt", "rgba", "-s", &format!("{w}x{h}"), "-i", "-", "-frames:v", "1"])
            .arg(out)
            .stdin(Stdio::piped())
            .spawn()
            .context("failed to start ffmpeg")?;
        let stdin = child.stdin.take();
        Ok(Self { child, stdin })
    }
}
