use std::{env, ffi, fmt, io, process, string};

struct ReadVersionError(String);

impl fmt::Debug for ReadVersionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("failed to detect rustc version by ")?;
        f.write_str(self.0.as_str())
    }
}

impl From<io::Error> for ReadVersionError {
    fn from(value: io::Error) -> Self {
        Self(format!("command I/O: {}", value))
    }
}

impl From<string::FromUtf8Error> for ReadVersionError {
    fn from(value: string::FromUtf8Error) -> Self {
        Self(format!("decoding command result: {}", value))
    }
}

impl From<String> for ReadVersionError {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl fmt::Display for ReadVersionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self, f)
    }
}

impl std::error::Error for ReadVersionError {}

fn read_version(rustc: ffi::OsString) -> Result<(u16, u16), ReadVersionError> {
    let process::Output { status, stdout, .. } = process::Command::new(rustc)
        .args(["--version", "--verbose"])
        .output()?;

    if !status.success() {
        return Err(format!("the status of command result: {}", status).into());
    }

    let output = String::from_utf8(stdout)?;
    let mut version = "";

    // Find the release line in the verbose version output.
    for line in output.lines() {
        if let Some(v) = line.strip_prefix("release: ") {
            version = v;
            break;
        }
    }

    if version.is_empty() {
        return Err(format!("finding release line: {}", output).into());
    }

    // build.rs don't need patch version and edition postfix.
    let mut iter = version.splitn(3, '.');

    if let (Some(major), Some(minor)) = (iter.next(), iter.next()) {
        let major = match major.parse() {
            Ok(t) => t,
            Err(e) => return Err(format!("major: {} {}", major, e).into()),
        };
        let minor = match minor.parse() {
            Ok(t) => t,
            Err(e) => return Err(format!("minor: {} {}", minor, e).into()),
        };

        Ok((major, minor))
    } else {
        Err(format!("spliting: {}", version).into())
    }
}

fn main() {
    let rustc = env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let version = match read_version(rustc) {
        Ok(v) => v,
        Err(e) => {
            println!("cargo:warning=latches: {}", e);
            return;
        }
    };

    if version < (1, 63) {
        println!("cargo:rustc-cfg=latches_no_const_sync");
    }
}
