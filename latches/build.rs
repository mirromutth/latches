fn main() {
    let rustc = std::env::var_os("RUSTC").unwrap_or("rustc".into());
    let output = std::process::Command::new(rustc)
        .arg("-vV")
        .output()
        .expect("Failed to execute `rustc -vV`");

    if !output.status.success() {
        return println!(
            "cargo:warning=latches: Failed `rustc -vV`: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let output = String::from_utf8_lossy(&output.stdout);

    for line in output.lines() {
        if let Some(version) = line.strip_prefix("release: ") {
            if let Some(v) = version_tuple(version) {
                if v < (1, 63) {
                    println!("cargo:rustc-cfg=latches_no_const_sync");
                }
                return;
            } else {
                println!("cargo:warning=latches: Unexpected version: {line}");
            }
        }
    }

    println!("cargo:warning=latches: No version line in `rustc -vV` output");
}

fn version_tuple(version: &str) -> Option<(u16, u16)> {
    let mut v = version.splitn(3, '.');

    if let (Some(f), Some(s)) = (v.next(), v.next()) {
        if let (Ok(major), Ok(minor)) = (f.parse(), s.parse()) {
            return Some((major, minor));
        }
    }

    None
}
