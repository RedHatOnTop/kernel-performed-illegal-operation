//! PS/2 Mouse Driver
//!
//! Initialize and configure PS/2 mouse for GUI input.

use x86_64::instructions::port::Port;

/// PS/2 controller ports
const PS2_DATA: u16 = 0x60;
const PS2_STATUS: u16 = 0x64;
const PS2_COMMAND: u16 = 0x64;

/// PS/2 controller commands
const CMD_READ_CONFIG: u8 = 0x20;
const CMD_WRITE_CONFIG: u8 = 0x60;
const CMD_DISABLE_MOUSE: u8 = 0xA7;
const CMD_ENABLE_MOUSE: u8 = 0xA8;
const CMD_MOUSE_PREFIX: u8 = 0xD4;

/// Mouse commands
const MOUSE_SET_DEFAULTS: u8 = 0xF6;
const MOUSE_ENABLE_STREAMING: u8 = 0xF4;

/// Wait for PS/2 controller to be ready for input
fn wait_write() {
    let mut status_port: Port<u8> = Port::new(PS2_STATUS);
    for _ in 0..10000 {
        let status = unsafe { status_port.read() };
        if status & 0x02 == 0 {
            return;
        }
        core::hint::spin_loop();
    }
}

/// Wait for PS/2 controller to have data available
fn wait_read() -> bool {
    let mut status_port: Port<u8> = Port::new(PS2_STATUS);
    for _ in 0..10000 {
        let status = unsafe { status_port.read() };
        if status & 0x01 != 0 {
            return true;
        }
        core::hint::spin_loop();
    }
    false
}

/// Write command to PS/2 controller
fn write_command(cmd: u8) {
    wait_write();
    let mut cmd_port: Port<u8> = Port::new(PS2_COMMAND);
    unsafe { cmd_port.write(cmd) };
}

/// Write data to PS/2 controller
fn write_data(data: u8) {
    wait_write();
    let mut data_port: Port<u8> = Port::new(PS2_DATA);
    unsafe { data_port.write(data) };
}

/// Read data from PS/2 controller
fn read_data() -> Option<u8> {
    if wait_read() {
        let mut data_port: Port<u8> = Port::new(PS2_DATA);
        Some(unsafe { data_port.read() })
    } else {
        None
    }
}

/// Send command to mouse (through controller)
fn mouse_write(cmd: u8) -> Option<u8> {
    write_command(CMD_MOUSE_PREFIX);
    write_data(cmd);
    read_data() // ACK
}

/// Initialize PS/2 mouse
pub fn init() {
    crate::serial_println!("[MOUSE] Initializing PS/2 mouse...");

    // Enable mouse port
    write_command(CMD_ENABLE_MOUSE);

    // Read controller configuration
    write_command(CMD_READ_CONFIG);
    let config = read_data().unwrap_or(0);

    // Enable mouse interrupt (bit 1) and mouse clock (clear bit 5)
    let new_config = (config | 0x02) & !0x20;
    write_command(CMD_WRITE_CONFIG);
    write_data(new_config);

    // Set defaults
    if let Some(ack) = mouse_write(MOUSE_SET_DEFAULTS) {
        crate::serial_println!("[MOUSE] Set defaults ACK: {:#x}", ack);
    }

    // Enable data streaming
    if let Some(ack) = mouse_write(MOUSE_ENABLE_STREAMING) {
        crate::serial_println!("[MOUSE] Enable streaming ACK: {:#x}", ack);
    }

    crate::serial_println!("[MOUSE] PS/2 mouse initialized");
}
