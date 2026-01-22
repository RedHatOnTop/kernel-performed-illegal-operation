//! Serial port driver for console output.
//!
//! This module provides serial console output using the 16550 UART.

use core::fmt;
use core::fmt::Write;
use spin::Mutex;
use uart_16550::SerialPort;

/// COM1 base address.
const COM1_BASE: u16 = 0x3F8;

/// COM2 base address.
const COM2_BASE: u16 = 0x2F8;

/// Global serial port (COM1).
static SERIAL1: Mutex<Option<SerialPort>> = Mutex::new(None);

/// Global serial port (COM2).
static SERIAL2: Mutex<Option<SerialPort>> = Mutex::new(None);

/// Initialize the serial ports.
pub fn init() {
    init_com1();
    init_com2();
}

/// Initialize COM1.
fn init_com1() {
    let mut port = unsafe { SerialPort::new(COM1_BASE) };
    port.init();
    *SERIAL1.lock() = Some(port);
}

/// Initialize COM2.
fn init_com2() {
    let mut port = unsafe { SerialPort::new(COM2_BASE) };
    port.init();
    *SERIAL2.lock() = Some(port);
}

/// Write a byte to COM1.
pub fn write_byte(byte: u8) {
    if let Some(ref mut serial) = *SERIAL1.lock() {
        serial.send(byte);
    }
}

/// Write a string to COM1.
pub fn write_str(s: &str) {
    for byte in s.bytes() {
        write_byte(byte);
    }
}

/// Read a byte from COM1 (blocking).
pub fn read_byte() -> u8 {
    loop {
        if let Some(ref mut serial) = *SERIAL1.lock() {
            if let Some(byte) = try_read_byte_impl(serial) {
                return byte;
            }
        }
        // Yield to prevent busy waiting
        core::hint::spin_loop();
    }
}

/// Try to read a byte from COM1 (non-blocking).
pub fn try_read_byte() -> Option<u8> {
    if let Some(ref mut serial) = *SERIAL1.lock() {
        try_read_byte_impl(serial)
    } else {
        None
    }
}

/// Internal implementation for non-blocking read.
fn try_read_byte_impl(serial: &mut SerialPort) -> Option<u8> {
    // Check if data is available (line status register bit 0)
    let lsr = unsafe { x86_64::instructions::port::Port::<u8>::new(COM1_BASE + 5).read() };
    if lsr & 1 != 0 {
        Some(serial.receive())
    } else {
        None
    }
}

/// Serial writer for formatting.
pub struct SerialWriter;

impl fmt::Write for SerialWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        write_str(s);
        Ok(())
    }
}

/// Print macro for serial output.
#[macro_export]
macro_rules! serial_print {
    ($($arg:tt)*) => {
        $crate::serial::_print(format_args!($($arg)*));
    };
}

/// Println macro for serial output.
#[macro_export]
macro_rules! serial_println {
    () => ($crate::serial_print!("\n"));
    ($($arg:tt)*) => {
        $crate::serial_print!("{}\n", format_args!($($arg)*));
    };
}

/// Internal print function.
#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    use x86_64::instructions::interrupts;

    // Disable interrupts to prevent deadlock
    interrupts::without_interrupts(|| {
        SerialWriter.write_fmt(args).unwrap();
    });
}

/// Log levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    /// Trace level (most verbose).
    Trace = 0,
    /// Debug level.
    Debug = 1,
    /// Info level.
    Info = 2,
    /// Warning level.
    Warn = 3,
    /// Error level.
    Error = 4,
}

/// Current log level.
static LOG_LEVEL: Mutex<LogLevel> = Mutex::new(LogLevel::Info);

/// Set the log level.
pub fn set_log_level(level: LogLevel) {
    *LOG_LEVEL.lock() = level;
}

/// Get the current log level.
pub fn log_level() -> LogLevel {
    *LOG_LEVEL.lock()
}

/// Log a message at the given level.
pub fn log(level: LogLevel, args: fmt::Arguments) {
    if level >= log_level() {
        let prefix = match level {
            LogLevel::Trace => "[TRACE]",
            LogLevel::Debug => "[DEBUG]",
            LogLevel::Info => "[INFO ]",
            LogLevel::Warn => "[WARN ]",
            LogLevel::Error => "[ERROR]",
        };
        serial_println!("{} {}", prefix, args);
    }
}

/// Log macros.
#[macro_export]
macro_rules! trace {
    ($($arg:tt)*) => {
        $crate::serial::log($crate::serial::LogLevel::Trace, format_args!($($arg)*));
    };
}

#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => {
        $crate::serial::log($crate::serial::LogLevel::Debug, format_args!($($arg)*));
    };
}

#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {
        $crate::serial::log($crate::serial::LogLevel::Info, format_args!($($arg)*));
    };
}

#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => {
        $crate::serial::log($crate::serial::LogLevel::Warn, format_args!($($arg)*));
    };
}

#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {
        $crate::serial::log($crate::serial::LogLevel::Error, format_args!($($arg)*));
    };
}
