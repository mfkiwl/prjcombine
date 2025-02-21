use std::{
    error::Error,
    fs::{create_dir_all, read_to_string},
    io::Write,
    process::Stdio,
};

use crate::vm6::Vm6;
use prjcombine_re_toolchain::Toolchain;
use simple_error::bail;

pub fn run_tsim(tc: &Toolchain, vm6: &Vm6) -> Result<(String, String), Box<dyn Error>> {
    let dir = tempfile::Builder::new()
        .prefix("prjcombine_xilinx_recpld_tsim")
        .tempdir()?;

    let mut vs = String::new();
    vm6.write(&mut vs)?;
    std::fs::write(dir.path().join("t.vm6"), &vs)?;

    let mut cmd = tc.command("tsim");
    cmd.current_dir(dir.path().as_os_str());
    cmd.stdin(Stdio::null());
    cmd.arg("t.vm6");
    let status = cmd.output()?;
    if !status.status.success() {
        let _ = std::io::stderr().write_all(&status.stdout);
        let _ = std::io::stderr().write_all(&status.stderr);
        let _ = create_dir_all("crash");
        let fname = format!(
            "crash/{part}-{pid}-{r}.vm6",
            part = vm6.part,
            pid = std::process::id(),
            r = rand::random::<u64>(),
        );
        let _ = std::fs::write(fname, vs);
        std::mem::forget(dir);
        bail!("non-zero tsim status");
    }

    let mut cmd = tc.command("netgen");
    cmd.current_dir(dir.path().as_os_str());
    cmd.stdin(Stdio::null());
    cmd.arg("-sim");
    cmd.arg("t.nga");
    cmd.arg("-ofmt");
    cmd.arg("verilog");
    let status = cmd.output()?;
    if !status.status.success() {
        let _ = std::io::stderr().write_all(&status.stdout);
        let _ = std::io::stderr().write_all(&status.stderr);
        let _ = create_dir_all("crash");
        let fname = format!(
            "crash/{part}-{pid}-{r}.vm6",
            part = vm6.part,
            pid = std::process::id(),
            r = rand::random::<u64>(),
        );
        let _ = std::fs::write(fname, vs);
        std::mem::forget(dir);
        bail!("non-zero tsim status");
    }

    let v = read_to_string(dir.path().join("t.v"))?;
    let sdf = read_to_string(dir.path().join("t.sdf"))?;
    Ok((v, sdf))
}
