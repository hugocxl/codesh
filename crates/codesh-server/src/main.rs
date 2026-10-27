//! Server executable composition root and process lifecycle.

mod args;
mod logging;
mod remote_listener;
mod run;
mod settings_file;

use std::io::{self, Write};
use std::process::ExitCode;

fn main() -> ExitCode {
    match execute() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            log::error!("{error}");
            let _ = writeln!(io::stderr(), "codesh-server: {error}");
            ExitCode::FAILURE
        }
    }
}

fn execute() -> io::Result<()> {
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    match arguments.as_slice() {
        [flag] if flag == "--build-info" => {
            serde_json::to_writer(io::stdout().lock(), &codesh_protocol::BuildInfo::current())?;
            Ok(())
        }
        [flag] if flag == "--version" || flag == "-V" => {
            writeln!(io::stdout(), "codesh-server {}", env!("CARGO_PKG_VERSION"))
        }
        [flag] if flag == "--help" || flag == "-h" => writeln!(
            io::stdout(),
            "Usage: codesh-server [--socket PATH --settings PATH --log PATH]\n       codesh-server --help | --version | --build-info"
        ),
        _ => {
            let args = args::Args::parse(arguments)?;
            let _lease = codesh_core::bundle::acquire_runtime(
                &codesh_core::executable::current_path()?,
                &serde_json::to_vec(&codesh_protocol::BuildInfo::current())?,
            )?;
            run::run(&args)
        }
    }
}
