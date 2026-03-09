//! Lock-free work queue for deferred interrupt processing.
//!
//! ISR handlers push work items into this queue (lock-free, interrupt-safe).
//! The main loop drains the queue and dispatches events to registered callbacks
//! with interrupts enabled — avoiding the deadlock caused by acquiring Mutex
//! locks inside interrupt handlers.
//!
//! # Design
//!
//! - Fixed-size ring buffer (power of 2) using atomic head/tail indices.
//! - Single producer per interrupt vector, single consumer (main loop).
//! - `push()` silently drops items if the queue is full (back-pressure).
//! - `drain()` processes all pending items in FIFO order.

use core::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};

/// Maximum number of pending work items.
/// Must be a power of 2 for efficient modular indexing.
const QUEUE_SIZE: usize = 256;
const QUEUE_MASK: usize = QUEUE_SIZE - 1;

/// A work item produced by an ISR and consumed by the main loop.
#[derive(Clone, Copy)]
pub enum WorkItem {
    /// Timer tick event.
    TimerTick,
    /// Keyboard event: (character, scancode, pressed, ctrl, shift, alt).
    KeyEvent(char, u8, bool, bool, bool, bool),
    /// Mouse byte received from PS/2.
    MouseByte(u8),
}

/// The work item ring buffer.
///
/// Uses `MaybeUninit`-style storage with atomic indices.
/// Items are stored as `u128` to avoid alignment issues.
static mut QUEUE_STORAGE: [WorkItemStorage; QUEUE_SIZE] = [WorkItemStorage::EMPTY; QUEUE_SIZE];

/// Atomic write index (tail — where producers write).
static WRITE_IDX: AtomicUsize = AtomicUsize::new(0);

/// Atomic read index (head — where the consumer reads).
static READ_IDX: AtomicUsize = AtomicUsize::new(0);

/// Callback function pointers (set once during init, read from drain).
static KEY_CALLBACK: AtomicPtr<()> = AtomicPtr::new(core::ptr::null_mut());
static MOUSE_CALLBACK: AtomicPtr<()> = AtomicPtr::new(core::ptr::null_mut());
static TIMER_CALLBACK: AtomicPtr<()> = AtomicPtr::new(core::ptr::null_mut());

/// Keyboard event callback type: (char, scancode, pressed, ctrl, shift, alt).
pub type KeyCallback = fn(char, u8, bool, bool, bool, bool);
/// Mouse byte callback type.
pub type MouseByteCallback = fn(u8);
/// Timer callback type.
pub type TimerCallback = fn();

/// Compact storage for a WorkItem, avoiding enum discriminant issues.
#[derive(Clone, Copy)]
#[repr(C)]
struct WorkItemStorage {
    /// 0 = empty, 1 = TimerTick, 2 = KeyEvent, 3 = MouseByte
    tag: u8,
    scancode: u8,
    pressed: u8,
    ctrl: u8,
    shift: u8,
    alt: u8,
    _pad: [u8; 2],
    character: u32,
}

impl WorkItemStorage {
    const EMPTY: Self = Self {
        tag: 0,
        scancode: 0,
        pressed: 0,
        ctrl: 0,
        shift: 0,
        alt: 0,
        _pad: [0; 2],
        character: 0,
    };

    fn from_work_item(item: WorkItem) -> Self {
        match item {
            WorkItem::TimerTick => Self {
                tag: 1,
                ..Self::EMPTY
            },
            WorkItem::KeyEvent(ch, sc, pr, ct, sh, al) => Self {
                tag: 2,
                character: ch as u32,
                scancode: sc,
                pressed: pr as u8,
                ctrl: ct as u8,
                shift: sh as u8,
                alt: al as u8,
                _pad: [0; 2],
            },
            WorkItem::MouseByte(b) => Self {
                tag: 3,
                scancode: b,
                ..Self::EMPTY
            },
        }
    }

    fn to_work_item(self) -> Option<WorkItem> {
        match self.tag {
            1 => Some(WorkItem::TimerTick),
            2 => Some(WorkItem::KeyEvent(
                // SAFETY: we only store valid char values.
                unsafe { char::from_u32_unchecked(self.character) },
                self.scancode,
                self.pressed != 0,
                self.ctrl != 0,
                self.shift != 0,
                self.alt != 0,
            )),
            3 => Some(WorkItem::MouseByte(self.scancode)),
            _ => None,
        }
    }
}

/// Push a work item into the queue.
///
/// Called from ISR context — must be lock-free.
/// Drops the item silently if the queue is full.
pub fn push(item: WorkItem) {
    let write = WRITE_IDX.load(Ordering::Relaxed);
    let read = READ_IDX.load(Ordering::Acquire);

    // Check if full.
    if write.wrapping_sub(read) >= QUEUE_SIZE {
        // Queue full — drop item (back-pressure).
        return;
    }

    let slot = write & QUEUE_MASK;
    // SAFETY: single producer per slot (ISR is non-reentrant on x86).
    unsafe {
        QUEUE_STORAGE[slot] = WorkItemStorage::from_work_item(item);
    }

    // Publish the write.
    WRITE_IDX.store(write.wrapping_add(1), Ordering::Release);
}

/// Drain all pending work items and dispatch them to registered callbacks.
///
/// Called from the main loop with interrupts enabled.
/// Returns the number of items processed.
pub fn drain() -> usize {
    let mut count = 0;

    loop {
        let read = READ_IDX.load(Ordering::Relaxed);
        let write = WRITE_IDX.load(Ordering::Acquire);

        if read == write {
            break; // Queue empty.
        }

        let slot = read & QUEUE_MASK;
        // SAFETY: we are the sole consumer; the producer has published this slot.
        let storage = unsafe { QUEUE_STORAGE[slot] };
        READ_IDX.store(read.wrapping_add(1), Ordering::Release);

        if let Some(item) = storage.to_work_item() {
            dispatch(item);
            count += 1;
        }
    }

    count
}

/// Register a keyboard event callback.
pub fn register_key_callback(cb: KeyCallback) {
    KEY_CALLBACK.store(cb as *mut (), Ordering::Release);
}

/// Register a mouse byte callback.
pub fn register_mouse_callback(cb: MouseByteCallback) {
    MOUSE_CALLBACK.store(cb as *mut (), Ordering::Release);
}

/// Register a timer callback.
pub fn register_timer_callback(cb: TimerCallback) {
    TIMER_CALLBACK.store(cb as *mut (), Ordering::Release);
}

/// Dispatch a single work item to its registered callback.
fn dispatch(item: WorkItem) {
    match item {
        WorkItem::TimerTick => {
            let ptr = TIMER_CALLBACK.load(Ordering::Acquire);
            if !ptr.is_null() {
                // SAFETY: the pointer was set via register_timer_callback
                // with a valid fn() function pointer.
                let cb: TimerCallback = unsafe { core::mem::transmute(ptr) };
                cb();
            }
        }
        WorkItem::KeyEvent(ch, sc, pr, ct, sh, al) => {
            let ptr = KEY_CALLBACK.load(Ordering::Acquire);
            if !ptr.is_null() {
                // SAFETY: the pointer was set via register_key_callback
                // with a valid fn(...) function pointer.
                let cb: KeyCallback = unsafe { core::mem::transmute(ptr) };
                cb(ch, sc, pr, ct, sh, al);
            }
        }
        WorkItem::MouseByte(byte) => {
            let ptr = MOUSE_CALLBACK.load(Ordering::Acquire);
            if !ptr.is_null() {
                // SAFETY: the pointer was set via register_mouse_callback
                // with a valid fn(u8) function pointer.
                let cb: MouseByteCallback = unsafe { core::mem::transmute(ptr) };
                cb(byte);
            }
        }
    }
}

/// Discard all pending items without dispatching callbacks.
///
/// Advances READ_IDX to match WRITE_IDX, effectively emptying the
/// queue. Call this before entering the main drain loop to clear
/// stale items that accumulated during boot-time initialization
/// (avoids dispatching hundreds of accumulated timer tick callbacks
/// that would each trigger a full-screen framebuffer render).
pub fn reset() {
    let write = WRITE_IDX.load(Ordering::SeqCst);
    READ_IDX.store(write, Ordering::SeqCst);
}
