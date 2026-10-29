use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub(crate) enum Source {
    Release(String),
    Development(PathBuf),
}

impl Source {
    pub(crate) fn current() -> Self {
        crate::model::released_version().map_or_else(
            || {
                Self::Development(
                    Path::new(env!("CARGO_MANIFEST_DIR"))
                        .join("../../target/linux-dev/codesh-dev-linux-x86_64.tar.gz"),
                )
            },
            |version| Self::Release(version.into()),
        )
    }

    pub(crate) fn command(&self) -> String {
        match self {
            Self::Release(version) => format!(
                "sh -c 'curl -fsSL {}/v{version}/install-codesh.sh | sh -s -- --version {version}'",
                crate::updater::RELEASES
            ),
            Self::Development(_) => {
                let installer = include_str!("../../../scripts/install-codesh.sh");
                format!(
                    "sh -c '{}' codesh-install --dev-archive",
                    installer.replace('\'', "'\\''")
                )
            }
        }
    }

    pub(crate) fn archive(&self) -> Option<&Path> {
        match self {
            Self::Release(_) => None,
            Self::Development(archive) => Some(archive),
        }
    }

    pub(crate) fn description(&self, name: &str) -> String {
        match self {
            Self::Release(version) => format!(
                "Codesh {version} is downloaded from GitHub and installed in ~/.local/bin on {name}. It needs curl there, and builds are currently published for Apple Silicon Macs only."
            ),
            Self::Development(archive) => format!(
                "The development build at {} is uploaded to {}. It installs codesh-server and its codesh helper in ~/.local/bin, then tests the connection. It requires x86_64 Linux with glibc 2.35 or newer.",
                archive.display(),
                name
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_embedded_installer_survives_shell_quoting() {
        let source = Source::Development(PathBuf::from("unused"));
        let output = std::process::Command::new("/bin/sh")
            .args(["-c", &format!("{} --help", source.command())])
            .stdin(std::process::Stdio::null())
            .output()
            .expect("installer help");
        assert!(output.status.success());
        assert!(String::from_utf8_lossy(&output.stdout).starts_with("Usage: install-codesh.sh"));
    }
}
