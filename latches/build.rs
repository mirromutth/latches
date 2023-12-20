fn main() {
    let rustc = std::env::var_os("RUSTC").unwrap_or("rustc".into());
    let output = std::process::Command::new(rustc)
        .arg("-vV")
        .output()
        .expect("Failed to execute `rustc -vV`");

    if !output.status.success() {
        return println!(
            "cargo:warning=latches: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let output = String::from_utf8_lossy(&output.stdout);
    for line in output.lines() {
        if let Some(version) = line.strip_prefix("release: ") {
            if let Some((ver, _)) = version.rsplit_once('.') {
                if let Some(v) = into_tuple(ver) {
                    if v < (1, 63) {
                        println!("cargo:rustc-cfg=latches_no_const_sync");
                    }
                    return;
                }
            }
        }
    }
    println!(
        "cargo:warning=latches: Unexpected output from `rustc -vV`:\n{}",
        output
    )
}

fn into_tuple(version: &str) -> Option<(u16, u16)> {
    if let Some((major, minor)) = version.split_once('.') {
        if let (Ok(maj), Ok(min)) = (major.parse(), minor.parse()) {
            return Some((maj, min));
        }
    }
    None
}
