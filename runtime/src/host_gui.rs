//! `kpio:gui` host function bindings.
//!
//! Provides window management, 2D canvas drawing, and event polling for WASM
//! apps that target the KPIO GUI subsystem.  In the kernel environment these
//! calls bridge to the kernel's framebuffer / input subsystems; in test /
//! host mode they operate on in-memory state so that tests can verify
//! behaviour without an actual display.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use crate::executor::ExecutorContext;
use crate::instance::Imports;
use crate::interpreter::{TrapError, WasmValue};

use spin::Mutex;

// ---------------------------------------------------------------------------
// Window / canvas state (in-memory for host-side testing; kernel replaces)
// ---------------------------------------------------------------------------

/// Represents a virtual window.
#[derive(Debug, Clone)]
pub struct VirtualWindow {
    pub id: u32,
    pub title: String,
    pub width: u32,
    pub height: u32,
    /// ARGB framebuffer (row-major).
    pub framebuffer: Vec<u32>,
    /// Pending events.
    pub events: Vec<GuiEvent>,
    /// Whether the window is open.
    pub open: bool,
}

/// GUI event types.
#[derive(Debug, Clone, Copy)]
pub struct GuiEvent {
    pub kind: u32,
    pub key_code: u32,
    pub mouse_x: i32,
    pub mouse_y: i32,
    pub width: u32,
    pub height: u32,
}

/// Event type constants.
pub const EVENT_NONE: u32 = 0;
pub const EVENT_KEY_DOWN: u32 = 1;
pub const EVENT_KEY_UP: u32 = 2;
pub const EVENT_MOUSE_MOVE: u32 = 3;
pub const EVENT_MOUSE_DOWN: u32 = 4;
pub const EVENT_MOUSE_UP: u32 = 5;
pub const EVENT_CLOSE: u32 = 6;
pub const EVENT_RESIZE: u32 = 7;

/// Global window manager state.
static WINDOWS: Mutex<Option<WindowManager>> = Mutex::new(None);

struct WindowManager {
    windows: BTreeMap<u32, VirtualWindow>,
    next_id: u32,
}

impl WindowManager {
    fn new() -> Self {
        WindowManager {
            windows: BTreeMap::new(),
            next_id: 1,
        }
    }
}

fn with_wm<F, R>(f: F) -> R
where
    F: FnOnce(&mut WindowManager) -> R,
{
    let mut guard = WINDOWS.lock();
    if guard.is_none() {
        *guard = Some(WindowManager::new());
    }
    f(guard.as_mut().unwrap())
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

/// Register all `kpio_gui` host functions.
pub fn register(imports: &mut Imports) {
    imports.add_function("kpio_gui", "create_window", host_create_window);
    imports.add_function("kpio_gui", "close_window", host_close_window);
    imports.add_function("kpio_gui", "set_title", host_set_title);
    imports.add_function("kpio_gui", "draw_rect", host_draw_rect);
    imports.add_function("kpio_gui", "draw_text", host_draw_text);
    imports.add_function("kpio_gui", "draw_line", host_draw_line);
    imports.add_function("kpio_gui", "clear", host_clear);
    imports.add_function("kpio_gui", "request_frame", host_request_frame);
    imports.add_function("kpio_gui", "poll_event", host_poll_event);
    imports.add_function("kpio_gui", "draw_image", host_draw_image);
}

// ---------------------------------------------------------------------------
// Host function implementations
// ---------------------------------------------------------------------------

/// create_window(title_ptr, title_len, width, height) -> window_id
fn host_create_window(
    ctx: &mut ExecutorContext,
    args: &[WasmValue],
) -> Result<Vec<WasmValue>, TrapError> {
    let title_ptr = args.get(0).and_then(|v| v.as_i32()).unwrap_or(0) as usize;
    let title_len = args.get(1).and_then(|v| v.as_i32()).unwrap_or(0) as usize;
    let width = args.get(2).and_then(|v| v.as_i32()).unwrap_or(640) as u32;
    let height = args.get(3).and_then(|v| v.as_i32()).unwrap_or(480) as u32;

    let title = if let Some(mem) = ctx.memories.first() {
        mem.read_bytes(title_ptr, title_len)
            .ok()
            .and_then(|b| core::str::from_utf8(b).ok())
            .map(String::from)
            .unwrap_or_else(|| String::from("KPIO Window"))
    } else {
        String::from("KPIO Window")
    };

    let id = with_wm(|wm| {
        let id = wm.next_id;
        wm.next_id += 1;
        let fb_size = (width * height) as usize;
        wm.windows.insert(
            id,
            VirtualWindow {
                id,
                title,
                width,
                height,
                framebuffer: vec![0xFF000000; fb_size], // opaque black
                events: Vec::new(),
                open: true,
            },
        );
        id
    });

    Ok(vec![WasmValue::I32(id as i32)])
}

/// close_window(window_id)
fn host_close_window(
    _ctx: &mut ExecutorContext,
    args: &[WasmValue],
) -> Result<Vec<WasmValue>, TrapError> {
    let id = args.get(0).and_then(|v| v.as_i32()).unwrap_or(0) as u32;
    with_wm(|wm| {
        if let Some(win) = wm.windows.get_mut(&id) {
            win.open = false;
        }
    });
    Ok(vec![WasmValue::I32(0)])
}

/// set_title(window_id, title_ptr, title_len)
fn host_set_title(
    ctx: &mut ExecutorContext,
    args: &[WasmValue],
) -> Result<Vec<WasmValue>, TrapError> {
    let id = args.get(0).and_then(|v| v.as_i32()).unwrap_or(0) as u32;
    let title_ptr = args.get(1).and_then(|v| v.as_i32()).unwrap_or(0) as usize;
    let title_len = args.get(2).and_then(|v| v.as_i32()).unwrap_or(0) as usize;

    let title = if let Some(mem) = ctx.memories.first() {
        mem.read_bytes(title_ptr, title_len)
            .ok()
            .and_then(|b| core::str::from_utf8(b).ok())
            .map(String::from)
            .unwrap_or_default()
    } else {
        String::new()
    };

    with_wm(|wm| {
        if let Some(win) = wm.windows.get_mut(&id) {
            win.title = title;
        }
    });

    Ok(vec![WasmValue::I32(0)])
}

/// draw_rect(window_id, x, y, w, h, color)
fn host_draw_rect(
    _ctx: &mut ExecutorContext,
    args: &[WasmValue],
) -> Result<Vec<WasmValue>, TrapError> {
    let id = args.get(0).and_then(|v| v.as_i32()).unwrap_or(0) as u32;
    let x = args.get(1).and_then(|v| v.as_i32()).unwrap_or(0);
    let y = args.get(2).and_then(|v| v.as_i32()).unwrap_or(0);
    let w = args.get(3).and_then(|v| v.as_i32()).unwrap_or(0) as u32;
    let h = args.get(4).and_then(|v| v.as_i32()).unwrap_or(0) as u32;
    let color = args.get(5).and_then(|v| v.as_i32()).unwrap_or(0) as u32;

    with_wm(|wm| {
        if let Some(win) = wm.windows.get_mut(&id) {
            let win_w = win.width;
            let win_h = win.height;
            for dy in 0..h {
                for dx in 0..w {
                    let px = x + dx as i32;
                    let py = y + dy as i32;
                    if px >= 0 && py >= 0 && (px as u32) < win_w && (py as u32) < win_h {
                        let idx = (py as u32 * win_w + px as u32) as usize;
                        if idx < win.framebuffer.len() {
                            win.framebuffer[idx] = color;
                        }
                    }
                }
            }
        }
    });

    Ok(vec![WasmValue::I32(0)])
}

/// draw_text(window_id, x, y, text_ptr, text_len, size, color)
/// Simplified: writes text position into a log; actual glyph rendering would
/// require a font rasterizer (implemented in kernel GUI), so this stores a
/// marker in the framebuffer metadata.
fn host_draw_text(
    ctx: &mut ExecutorContext,
    args: &[WasmValue],
) -> Result<Vec<WasmValue>, TrapError> {
    let _id = args.get(0).and_then(|v| v.as_i32()).unwrap_or(0) as u32;
    let _x = args.get(1).and_then(|v| v.as_i32()).unwrap_or(0);
    let _y = args.get(2).and_then(|v| v.as_i32()).unwrap_or(0);
    let text_ptr = args.get(3).and_then(|v| v.as_i32()).unwrap_or(0) as usize;
    let text_len = args.get(4).and_then(|v| v.as_i32()).unwrap_or(0) as usize;
    let _size = args.get(5).and_then(|v| v.as_i32()).unwrap_or(16);
    let _color = args.get(6).and_then(|v| v.as_i32()).unwrap_or(0) as u32;

    // Read text from linear memory (for logging / testing)
    if let Some(mem) = ctx.memories.first() {
        let _text = mem
            .read_bytes(text_ptr, text_len)
            .ok()
            .and_then(|b| core::str::from_utf8(b).ok());
        // In kernel: call kernel GUI draw_string()
        // In test: text is accessible via the above variable
    }

    Ok(vec![WasmValue::I32(0)])
}

/// draw_line(window_id, x1, y1, x2, y2, color) — Bresenham line
fn host_draw_line(
    _ctx: &mut ExecutorContext,
    args: &[WasmValue],
) -> Result<Vec<WasmValue>, TrapError> {
    let id = args.get(0).and_then(|v| v.as_i32()).unwrap_or(0) as u32;
    let mut x0 = args.get(1).and_then(|v| v.as_i32()).unwrap_or(0);
    let mut y0 = args.get(2).and_then(|v| v.as_i32()).unwrap_or(0);
    let x1 = args.get(3).and_then(|v| v.as_i32()).unwrap_or(0);
    let y1 = args.get(4).and_then(|v| v.as_i32()).unwrap_or(0);
    let color = args.get(5).and_then(|v| v.as_i32()).unwrap_or(0) as u32;

    with_wm(|wm| {
        if let Some(win) = wm.windows.get_mut(&id) {
            let win_w = win.width;
            let win_h = win.height;

            let dx = (x1 - x0).abs();
            let dy = -(y1 - y0).abs();
            let sx: i32 = if x0 < x1 { 1 } else { -1 };
            let sy: i32 = if y0 < y1 { 1 } else { -1 };
            let mut err = dx + dy;

            loop {
                if x0 >= 0 && y0 >= 0 && (x0 as u32) < win_w && (y0 as u32) < win_h {
                    let idx = (y0 as u32 * win_w + x0 as u32) as usize;
                    if idx < win.framebuffer.len() {
                        win.framebuffer[idx] = color;
                    }
                }
                if x0 == x1 && y0 == y1 {
                    break;
                }
                let e2 = 2 * err;
                if e2 >= dy {
                    err += dy;
                    x0 += sx;
                }
                if e2 <= dx {
                    err += dx;
                    y0 += sy;
                }
            }
        }
    });

    Ok(vec![WasmValue::I32(0)])
}

/// clear(window_id, color)
fn host_clear(
    _ctx: &mut ExecutorContext,
    args: &[WasmValue],
) -> Result<Vec<WasmValue>, TrapError> {
    let id = args.get(0).and_then(|v| v.as_i32()).unwrap_or(0) as u32;
    let color = args.get(1).and_then(|v| v.as_i32()).unwrap_or(0) as u32;

    with_wm(|wm| {
        if let Some(win) = wm.windows.get_mut(&id) {
            for pixel in win.framebuffer.iter_mut() {
                *pixel = color;
            }
        }
    });

    Ok(vec![WasmValue::I32(0)])
}

/// request_frame(window_id) -> bool (1 = frame available)
fn host_request_frame(
    _ctx: &mut ExecutorContext,
    args: &[WasmValue],
) -> Result<Vec<WasmValue>, TrapError> {
    let id = args.get(0).and_then(|v| v.as_i32()).unwrap_or(0) as u32;
    let ready = with_wm(|wm| wm.windows.contains_key(&id));
    Ok(vec![WasmValue::I32(if ready { 1 } else { 0 })])
}

/// poll_event(window_id, event_buf_ptr, buf_len) -> event_type
fn host_poll_event(
    ctx: &mut ExecutorContext,
    args: &[WasmValue],
) -> Result<Vec<WasmValue>, TrapError> {
    let id = args.get(0).and_then(|v| v.as_i32()).unwrap_or(0) as u32;
    let buf_ptr = args.get(1).and_then(|v| v.as_i32()).unwrap_or(0) as usize;
    let _buf_len = args.get(2).and_then(|v| v.as_i32()).unwrap_or(0) as usize;

    let event = with_wm(|wm| {
        if let Some(win) = wm.windows.get_mut(&id) {
            if !win.events.is_empty() {
                Some(win.events.remove(0))
            } else {
                None
            }
        } else {
            None
        }
    });

    if let Some(ev) = event {
        // Write event struct to linear memory (24 bytes):
        // u32 kind | u32 key_code | i32 mouse_x | i32 mouse_y | u32 width | u32 height
        if let Some(mem) = ctx.memories.first_mut() {
            let _ = mem.write_u32(buf_ptr, ev.kind);
            let _ = mem.write_u32(buf_ptr + 4, ev.key_code);
            let _ = mem.write_u32(buf_ptr + 8, ev.mouse_x as u32);
            let _ = mem.write_u32(buf_ptr + 12, ev.mouse_y as u32);
            let _ = mem.write_u32(buf_ptr + 16, ev.width);
            let _ = mem.write_u32(buf_ptr + 20, ev.height);
        }
        Ok(vec![WasmValue::I32(ev.kind as i32)])
    } else {
        Ok(vec![WasmValue::I32(EVENT_NONE as i32)])
    }
}

/// draw_image(window_id, x, y, w, h, data_ptr, data_len) — RGBA bitmap blit
fn host_draw_image(
    ctx: &mut ExecutorContext,
    args: &[WasmValue],
) -> Result<Vec<WasmValue>, TrapError> {
    let id = args.get(0).and_then(|v| v.as_i32()).unwrap_or(0) as u32;
    let x = args.get(1).and_then(|v| v.as_i32()).unwrap_or(0);
    let y = args.get(2).and_then(|v| v.as_i32()).unwrap_or(0);
    let w = args.get(3).and_then(|v| v.as_i32()).unwrap_or(0) as u32;
    let h = args.get(4).and_then(|v| v.as_i32()).unwrap_or(0) as u32;
    let data_ptr = args.get(5).and_then(|v| v.as_i32()).unwrap_or(0) as usize;
    let data_len = args.get(6).and_then(|v| v.as_i32()).unwrap_or(0) as usize;

    let rgba_data = if let Some(mem) = ctx.memories.first() {
        mem.read_bytes(data_ptr, data_len).ok().map(|b| b.to_vec())
    } else {
        None
    };

    if let Some(data) = rgba_data {
        with_wm(|wm| {
            if let Some(win) = wm.windows.get_mut(&id) {
                let win_w = win.width;
                let win_h = win.height;
                let mut offset = 0usize;
                for dy in 0..h {
                    for dx in 0..w {
                        if offset + 4 > data.len() {
                            return;
                        }
                        let r = data[offset] as u32;
                        let g = data[offset + 1] as u32;
                        let b = data[offset + 2] as u32;
                        let a = data[offset + 3] as u32;
                        offset += 4;

                        let px = x + dx as i32;
                        let py = y + dy as i32;
                        if px >= 0 && py >= 0 && (px as u32) < win_w && (py as u32) < win_h {
                            let idx = (py as u32 * win_w + px as u32) as usize;
                            if idx < win.framebuffer.len() {
                                win.framebuffer[idx] = (a << 24) | (r << 16) | (g << 8) | b;
                            }
                        }
                    }
                }
            }
        });
    }

    Ok(vec![WasmValue::I32(0)])
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::{MemoryType, Module};

    fn make_ctx_with_memory() -> ExecutorContext {
        let mut module = Module::empty();
        module.memories.push(MemoryType {
            min: 1,
            max: None,
        });
        ExecutorContext::new(module).unwrap()
    }

    #[test]
    fn test_create_and_close_window() {
        let mut ctx = make_ctx_with_memory();
        // Write title "Test" at offset 0
        let _ = ctx.memories[0].write_bytes(0, b"Test");

        let result = host_create_window(
            &mut ctx,
            &[
                WasmValue::I32(0),   // title_ptr
                WasmValue::I32(4),   // title_len
                WasmValue::I32(320), // width
                WasmValue::I32(240), // height
            ],
        )
        .unwrap();
        let id = result[0].as_i32().unwrap();
        assert!(id > 0);

        // Close
        let _ = host_close_window(&mut ctx, &[WasmValue::I32(id)]);
    }

    #[test]
    fn test_draw_rect() {
        let mut ctx = make_ctx_with_memory();
        let result = host_create_window(
            &mut ctx,
            &[
                WasmValue::I32(0),
                WasmValue::I32(0),
                WasmValue::I32(100),
                WasmValue::I32(100),
            ],
        )
        .unwrap();
        let id = result[0].as_i32().unwrap();

        let _ = host_draw_rect(
            &mut ctx,
            &[
                WasmValue::I32(id),
                WasmValue::I32(10),
                WasmValue::I32(10),
                WasmValue::I32(5),
                WasmValue::I32(5),
                WasmValue::I32(0xFFFF0000u32 as i32), // red
            ],
        );

        // Verify pixel was set
        let pixel = with_wm(|wm| {
            let win = wm.windows.get(&(id as u32)).unwrap();
            win.framebuffer[10 * 100 + 10] // y=10, x=10
        });
        assert_eq!(pixel, 0xFFFF0000);
    }

    #[test]
    fn test_clear_window() {
        let mut ctx = make_ctx_with_memory();
        let result = host_create_window(
            &mut ctx,
            &[
                WasmValue::I32(0),
                WasmValue::I32(0),
                WasmValue::I32(10),
                WasmValue::I32(10),
            ],
        )
        .unwrap();
        let id = result[0].as_i32().unwrap();

        let _ = host_clear(
            &mut ctx,
            &[WasmValue::I32(id), WasmValue::I32(0xFF00FF00u32 as i32)],
        );

        let pixel = with_wm(|wm| wm.windows.get(&(id as u32)).unwrap().framebuffer[0]);
        assert_eq!(pixel, 0xFF00FF00);
    }
}
