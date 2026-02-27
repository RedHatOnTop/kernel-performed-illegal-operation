pub mod cli;
pub mod error;
pub mod health;
pub mod instance;
pub mod output;
pub mod qmp;
pub mod state;
pub mod store;
pub mod watchdog;

use std::process::ExitCode;

use clap::Parser;

use crate::cli::{Cli, Command};

// TODO: Remove allow once all handlers replace todo!() stubs
#[allow(unreachable_code, unused_variables)]
fn main() -> ExitCode {
    let cli = Cli::parse();

    let result: Result<serde_json::Value, error::KpioTestError> = match cli.command {
        Command::Create(args) => instance::create(args),
        Command::List => instance::list(),
        Command::Status(args) => instance::status(&args.name),
        Command::Serial(_args) => todo!("serial handler"),
        Command::SerialGrep(_args) => todo!("serial-grep handler"),
        Command::WaitFor(_args) => todo!("wait-for handler"),
        Command::SendCommand(_args) => todo!("send-command handler"),
        Command::Screenshot(_args) => todo!("screenshot handler"),
        Command::ScreenshotInterval(_args) => todo!("screenshot-interval handler"),
        Command::CompareScreenshot(_args) => todo!("compare-screenshot handler"),
        Command::ScreenOcr(_args) => todo!("screen-ocr handler"),
        Command::SendKey(_args) => todo!("send-key handler"),
        Command::TypeText(_args) => todo!("type-text handler"),
        Command::MouseClick(_args) => todo!("mouse-click handler"),
        Command::MouseMove(_args) => todo!("mouse-move handler"),
        Command::Snapshot(_args) => todo!("snapshot handler"),
        Command::GuestInfo(_args) => todo!("guest-info handler"),
        Command::PortForward(_args) => todo!("port-forward handler"),
        Command::CopyTo(_args) => todo!("copy-to handler"),
        Command::CopyFrom(_args) => todo!("copy-from handler"),
        Command::Logs(_args) => todo!("logs handler"),
        Command::Build(_args) => todo!("build handler"),
        Command::Verify(_args) => todo!("verify handler"),
        Command::Health => {
            let report = health::check(None);
            serde_json::to_value(&report).map_err(|e| error::KpioTestError::Json(e))
        }
        Command::DestroyAll => instance::destroy_all(),
        Command::Destroy(args) => instance::destroy(&args.name),
        Command::HelpCmd(_args) => todo!("help handler"),
    };

    match result {
        Ok(output) => {
            let _ = crate::output::emit(cli.output, &output);
            ExitCode::SUCCESS
        }
        Err(e) => {
            let code = e.exit_code();
            crate::output::emit_error(cli.output, exit_code_to_u8(&code), &e.to_string());
            code
        }
    }
}

fn exit_code_to_u8(code: &ExitCode) -> u8 {
    // ExitCode doesn't expose its inner value directly.
    // We compare against known values.
    if *code == ExitCode::from(2) {
        2
    } else if *code == ExitCode::from(1) {
        1
    } else {
        0
    }
}
