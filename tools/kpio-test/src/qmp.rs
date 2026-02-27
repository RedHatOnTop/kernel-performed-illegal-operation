//! QMP (QEMU Machine Protocol) client.
//!
//! Connects to a running QEMU instance via a Unix domain socket (or named
//! pipe on Windows), negotiates capabilities, and executes commands with a
//! 10-second default timeout. Asynchronous QMP events are silently skipped
//! while waiting for a command response.

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::error::KpioTestError;

/// Default timeout for QMP command responses.
const QMP_TIMEOUT: Duration = Duration::from_secs(10);

// ── QMP message types ────────────────────────────────────────────────

/// QMP greeting sent by QEMU on connection.
#[derive(Deserialize, Debug)]
pub struct QmpGreeting {
    #[serde(rename = "QMP")]
    pub qmp: QmpVersionBlock,
}

#[derive(Deserialize, Debug)]
pub struct QmpVersionBlock {
    pub version: QmpVersionInfo,
    pub capabilities: Vec<String>,
}

#[derive(Deserialize, Debug)]
pub struct QmpVersionInfo {
    pub qemu: QmpQemuVersion,
    pub package: String,
}

#[derive(Deserialize, Debug)]
pub struct QmpQemuVersion {
    pub major: u32,
    pub minor: u32,
    pub micro: u32,
}

/// Command sent to QEMU.
#[derive(Serialize, Debug)]
pub struct QmpCommand {
    pub execute: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<serde_json::Value>,
}

/// Successful QMP response.
#[derive(Deserialize, Debug)]
pub struct QmpSuccess {
    #[serde(rename = "return")]
    pub result: serde_json::Value,
}

/// QMP error response.
#[derive(Deserialize, Debug)]
pub struct QmpErrorResponse {
    pub error: QmpErrorDetail,
}

#[derive(Deserialize, Debug, Clone)]
pub struct QmpErrorDetail {
    pub class: String,
    pub desc: String,
}

/// Asynchronous QMP event (skipped during command execution).
#[derive(Deserialize, Debug)]
pub struct QmpEvent {
    pub event: String,
    pub data: Option<serde_json::Value>,
    pub timestamp: Option<QmpTimestamp>,
}

#[derive(Deserialize, Debug)]
pub struct QmpTimestamp {
    pub seconds: u64,
    pub microseconds: u64,
}

/// QMP status returned by `query-status`.
#[derive(Deserialize, Debug, Clone)]
pub struct QmpStatus {
    pub running: bool,
    pub status: String,
}

/// Input event for `input-send-event`.
#[derive(Serialize, Debug, Clone)]
pub struct InputEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    pub data: serde_json::Value,
}

// ── QMP client ───────────────────────────────────────────────────────

/// A QMP client connected to a single QEMU instance.
pub struct QmpClient {
    reader: BufReader<Box<dyn std::io::Read + Send>>,
    writer: Box<dyn std::io::Write + Send>,
}

impl QmpClient {
    /// Connect to a QMP socket, read the greeting, and negotiate capabilities.
    pub fn connect(socket_path: &Path) -> Result<Self, KpioTestError> {
        let (reader, writer) = Self::open_transport(socket_path)?;
        let mut client = QmpClient {
            reader: BufReader::new(reader),
            writer,
        };

        // Read QMP greeting
        let greeting_line = client.read_line()?;
        let _greeting: QmpGreeting = serde_json::from_str(&greeting_line).map_err(|e| {
            KpioTestError::QmpError {
                desc: format!("failed to parse QMP greeting: {e}"),
            }
        })?;

        // Negotiate capabilities
        client.send_raw(&serde_json::json!({"execute": "qmp_capabilities"}))?;
        let resp_line = client.read_line_skip_events()?;
        // Expect {"return": {}}
        let _: QmpSuccess = serde_json::from_str(&resp_line).map_err(|e| {
            KpioTestError::QmpError {
                desc: format!("qmp_capabilities negotiation failed: {e}"),
            }
        })?;

        Ok(client)
    }

    /// Execute a QMP command and return the parsed result.
    pub fn execute<T: serde::de::DeserializeOwned>(
        &mut self,
        command: &str,
        args: Option<serde_json::Value>,
    ) -> Result<T, KpioTestError> {
        let cmd = QmpCommand {
            execute: command.to_string(),
            arguments: args,
        };
        self.send_raw(&cmd)?;

        let resp_line = self.read_line_skip_events()?;

        // Try success first
        if let Ok(success) = serde_json::from_str::<QmpSuccess>(&resp_line) {
            let result: T = serde_json::from_value(success.result).map_err(|e| {
                KpioTestError::QmpError {
                    desc: format!("failed to parse QMP result for '{command}': {e}"),
                }
            })?;
            return Ok(result);
        }

        // Try error
        if let Ok(err_resp) = serde_json::from_str::<QmpErrorResponse>(&resp_line) {
            return Err(KpioTestError::QmpError {
                desc: format!("{}: {}", err_resp.error.class, err_resp.error.desc),
            });
        }

        Err(KpioTestError::QmpError {
            desc: format!("unexpected QMP response: {resp_line}"),
        })
    }

    /// Execute a QMP command that returns `{"return": {}}` (void result).
    pub fn execute_void(
        &mut self,
        command: &str,
        args: Option<serde_json::Value>,
    ) -> Result<(), KpioTestError> {
        let _: serde_json::Value = self.execute(command, args)?;
        Ok(())
    }

    // ── Convenience methods ──────────────────────────────────────────

    /// Capture a screenshot and save to the given path.
    pub fn screendump(&mut self, filename: &Path) -> Result<(), KpioTestError> {
        self.execute_void(
            "screendump",
            Some(serde_json::json!({
                "filename": filename.to_string_lossy()
            })),
        )
    }

    /// Send input events to the guest.
    pub fn input_send_event(&mut self, events: &[InputEvent]) -> Result<(), KpioTestError> {
        self.execute_void(
            "input-send-event",
            Some(serde_json::json!({ "events": events })),
        )
    }

    /// Save a VM snapshot with the given tag.
    pub fn savevm(&mut self, tag: &str) -> Result<(), KpioTestError> {
        self.human_monitor_command(&format!("savevm {tag}"))?;
        Ok(())
    }

    /// Restore a VM snapshot by tag.
    pub fn loadvm(&mut self, tag: &str) -> Result<(), KpioTestError> {
        self.human_monitor_command(&format!("loadvm {tag}"))?;
        Ok(())
    }

    /// Query the VM run status.
    pub fn query_status(&mut self) -> Result<QmpStatus, KpioTestError> {
        self.execute("query-status", None)
    }

    /// Execute a human monitor command and return the output string.
    pub fn human_monitor_command(&mut self, cmd: &str) -> Result<String, KpioTestError> {
        self.execute(
            "human-monitor-command",
            Some(serde_json::json!({"command-line": cmd})),
        )
    }

    // ── Transport helpers ────────────────────────────────────────────

    /// Open the platform-specific transport (Unix socket or Windows named pipe).
    #[cfg(unix)]
    fn open_transport(
        socket_path: &Path,
    ) -> Result<
        (
            Box<dyn std::io::Read + Send>,
            Box<dyn std::io::Write + Send>,
        ),
        KpioTestError,
    > {
        use std::os::unix::net::UnixStream;
        let stream = UnixStream::connect(socket_path).map_err(|e| KpioTestError::QmpError {
            desc: format!(
                "failed to connect to QMP socket at {}: {e}",
                socket_path.display()
            ),
        })?;
        stream.set_read_timeout(Some(QMP_TIMEOUT)).ok();
        stream.set_write_timeout(Some(QMP_TIMEOUT)).ok();
        let reader = stream.try_clone()?;
        Ok((Box::new(reader), Box::new(stream)))
    }

    #[cfg(windows)]
    fn open_transport(
        socket_path: &Path,
    ) -> Result<
        (
            Box<dyn std::io::Read + Send>,
            Box<dyn std::io::Write + Send>,
        ),
        KpioTestError,
    > {
        // On Windows, QMP uses a named pipe. We convert the socket path to a
        // pipe name: \\.\pipe\kpio-qmp-<instance-name>
        // However, QEMU on Windows can also use TCP sockets. For simplicity,
        // we try connecting as a regular file/pipe first.
        use std::fs::OpenOptions;
        let pipe = OpenOptions::new()
            .read(true)
            .write(true)
            .open(socket_path)
            .map_err(|e| KpioTestError::QmpError {
                desc: format!(
                    "failed to connect to QMP pipe at {}: {e}",
                    socket_path.display()
                ),
            })?;
        let reader = pipe.try_clone()?;
        Ok((Box::new(reader), Box::new(pipe)))
    }

    /// Read a single line from the QMP socket.
    fn read_line(&mut self) -> Result<String, KpioTestError> {
        let mut line = String::new();
        self.reader.read_line(&mut line).map_err(|e| {
            if e.kind() == std::io::ErrorKind::TimedOut
                || e.kind() == std::io::ErrorKind::WouldBlock
            {
                KpioTestError::QmpTimeout { seconds: 10 }
            } else {
                KpioTestError::QmpError {
                    desc: format!("QMP read error: {e}"),
                }
            }
        })?;
        if line.is_empty() {
            return Err(KpioTestError::QmpError {
                desc: "QMP connection closed unexpectedly".to_string(),
            });
        }
        Ok(line)
    }

    /// Read lines, skipping asynchronous QMP events, until a non-event line
    /// (success or error response) is received.
    fn read_line_skip_events(&mut self) -> Result<String, KpioTestError> {
        loop {
            let line = self.read_line()?;
            // Events have an "event" key at the top level
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&line) {
                if val.get("event").is_some() {
                    // This is an async event — skip it
                    continue;
                }
            }
            return Ok(line);
        }
    }

    /// Serialize and write a JSON value followed by a newline.
    fn send_raw<T: Serialize>(&mut self, value: &T) -> Result<(), KpioTestError> {
        let json = serde_json::to_string(value)?;
        self.writer
            .write_all(json.as_bytes())
            .map_err(|e| KpioTestError::QmpError {
                desc: format!("QMP write error: {e}"),
            })?;
        self.writer
            .write_all(b"\n")
            .map_err(|e| KpioTestError::QmpError {
                desc: format!("QMP write error: {e}"),
            })?;
        self.writer.flush().map_err(|e| KpioTestError::QmpError {
            desc: format!("QMP flush error: {e}"),
        })?;
        Ok(())
    }
}
