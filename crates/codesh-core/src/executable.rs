use std::io;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

pub fn command_arguments(source: &str) -> io::Result<Vec<String>> {
    let invalid = |message| io::Error::new(io::ErrorKind::InvalidInput, message);
    if source.chars().any(char::is_control) {
        return Err(invalid("Commands must not contain control characters"));
    }
    let mut arguments = Vec::new();
    let mut word = String::new();
    let mut started = false;
    let mut quote = None;
    let mut characters = source.chars();
    while let Some(character) = characters.next() {
        match (quote, character) {
            (Some('\''), '\'') | (Some('"'), '"') => quote = None,
            (None, '\'' | '"') => {
                quote = Some(character);
                started = true;
            }
            (None | Some('"'), '\\') => {
                let escaped = characters
                    .next()
                    .ok_or_else(|| invalid("Commands must not end with an unescaped backslash"))?;
                if quote == Some('"') && !matches!(escaped, '"' | '\\' | '$' | '`') {
                    word.push('\\');
                }
                word.push(escaped);
                started = true;
            }
            (None, character) if character.is_whitespace() => {
                if started {
                    arguments.push(std::mem::take(&mut word));
                    started = false;
                }
            }
            _ => {
                word.push(character);
                started = true;
            }
        }
    }
    if quote.is_some() {
        return Err(invalid("Close the quote in the command"));
    }
    if started {
        arguments.push(word);
    }
    if arguments.first().is_none_or(String::is_empty) {
        return Err(invalid("Enter a command with a non-empty program name"));
    }
    Ok(arguments)
}

pub fn current_path() -> io::Result<std::path::PathBuf> {
    // macOS current_exe can retain the launch symlink after its target changes.
    #[cfg(target_os = "macos")]
    let path = std::path::PathBuf::from(
        libproc::libproc::proc_pid::pidpath(
            i32::try_from(std::process::id()).map_err(io::Error::other)?,
        )
        .map_err(io::Error::other)?,
    );
    #[cfg(not(target_os = "macos"))]
    let path = std::env::current_exe()?;
    path.canonicalize()
}

pub fn build_metadata(executable: &Path) -> io::Result<Vec<u8>> {
    use std::io::{Read, Seek};
    let mut output = tempfile::tempfile()?;
    let mut child = Command::new(executable)
        .arg("--build-info")
        .stdin(Stdio::null())
        .stdout(output.try_clone()?)
        .stderr(Stdio::null())
        .spawn()?;
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if let Some(status) = child.try_wait()? {
            if !status.success() {
                return Err(io::Error::other("Server build metadata is unavailable"));
            }
            break;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "Server build metadata timed out",
            ));
        }
        thread::sleep(Duration::from_millis(10));
    }
    output.rewind()?;
    let mut bytes = Vec::new();
    output.take(4097).read_to_end(&mut bytes)?;
    if bytes.len() > 4096 {
        return Err(io::Error::other("Executable build metadata is too large"));
    }
    Ok(bytes)
}
