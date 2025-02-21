use crate::toolchain::Toolchain;
use nix::fcntl::{fcntl, FcntlArg};
use nix::sys::stat::Mode;
use nix::unistd::mkfifo;
use std::fs::{write, File};
use std::io::{self, BufReader, Read};
use std::os::unix::io::AsRawFd;
use std::process::{Child, Stdio};
use tempfile::TempDir;

pub struct ToolchainReader {
    _dir: TempDir,
    fifo: Option<File>,
    child: Child,
}

impl ToolchainReader {
    pub fn new(
        tc: &Toolchain,
        cmd: &str,
        args: &[&str],
        env: &[(&str, &str)],
        fifo_name: &str,
        input_files: &[(&str, &[u8])],
    ) -> Result<BufReader<Self>, Box<dyn std::error::Error>> {
        let dir = TempDir::new()?;
        for (k, v) in input_files {
            let path = dir.path().join(k);
            write(path, v)?;
        }
        let path = dir.path().join(fifo_name);
        mkfifo(&path, Mode::S_IRUSR | Mode::S_IWUSR)?;
        let mut cmd = tc.command(cmd);
        cmd.current_dir(dir.path().as_os_str());
        cmd.stdin(Stdio::null());
        cmd.stdout(Stdio::null());
        cmd.stderr(Stdio::null());
        for arg in args {
            cmd.arg(arg);
        }
        for (k, v) in env {
            cmd.env(k, v);
        }
        let child = cmd.spawn()?;
        let fifo = File::open(path)?;
        let _ = fcntl(fifo.as_raw_fd(), FcntlArg::F_SETPIPE_SZ(1 << 20));
        Ok(BufReader::new(ToolchainReader {
            fifo: Some(fifo),
            _dir: dir,
            child,
        }))
    }
}

impl Read for ToolchainReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match &mut self.fifo {
            Some(fifo) => fifo.read(buf),
            None => Ok(0),
        }
    }
}

impl Drop for ToolchainReader {
    fn drop(&mut self) {
        self.fifo = None;
        // Nothing much to do if it fails.
        let _ = self.child.wait();
    }
}
