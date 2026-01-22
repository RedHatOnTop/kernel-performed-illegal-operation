//! I/O APIC (Advanced Programmable Interrupt Controller) driver.
//!
//! The I/O APIC handles external interrupts from I/O devices and routes
//! them to the appropriate Local APIC(s). A system can have multiple
//! I/O APICs, each handling a range of Global System Interrupts (GSIs).
//!
//! # Registers
//!
//! The I/O APIC has two memory-mapped registers for indirect access:
//! - IOREGSEL (offset 0x00): Selects which internal register to access.
//! - IOWIN (offset 0x10): Data window for reading/writing selected register.
//!
//! # Redirection Table
//!
//! Each I/O APIC has 24 redirection table entries (RTEs), each 64 bits:
//! - Bits 0-7: Interrupt vector (32-255).
//! - Bits 8-10: Delivery mode.
//! - Bit 11: Destination mode (0=Physical, 1=Logical).
//! - Bit 12: Delivery status (RO).
//! - Bit 13: Interrupt pin polarity (0=Active high, 1=Active low).
//! - Bit 14: Remote IRR (RO).
//! - Bit 15: Trigger mode (0=Edge, 1=Level).
//! - Bit 16: Mask (1=Masked/disabled).
//! - Bits 56-63: Destination APIC ID.
//!
//! # References
//!
//! - Intel 82093AA I/O Advanced Programmable Interrupt Controller Datasheet
//! - ACPI Specification, MADT (Multiple APIC Description Table)

use core::ptr::{read_volatile, write_volatile};
use core::sync::atomic::{AtomicU64, Ordering};
use alloc::vec::Vec;
use spin::Mutex;

/// Default I/O APIC base address.
///
/// The actual address should be obtained from ACPI MADT.
const IOAPIC_BASE_DEFAULT: u64 = 0xFEC0_0000;

/// Maximum number of I/O APICs supported.
const MAX_IOAPICS: usize = 8;

/// Physical memory offset for MMIO access.
static PHYS_MEM_OFFSET: AtomicU64 = AtomicU64::new(0);

/// I/O APIC register offsets for indirect access.
mod regs {
    /// I/O Register Select.
    pub const IOREGSEL: u32 = 0x00;
    /// I/O Window (Data).
    pub const IOWIN: u32 = 0x10;
}

/// I/O APIC internal register indices.
mod internal {
    /// I/O APIC ID.
    pub const IOAPICID: u8 = 0x00;
    /// I/O APIC Version.
    pub const IOAPICVER: u8 = 0x01;
    /// I/O APIC Arbitration ID.
    pub const IOAPICARB: u8 = 0x02;
    /// Redirection Table base (entries 0-23 at 0x10-0x3F).
    pub const IOREDTBL_BASE: u8 = 0x10;
}

/// Interrupt delivery modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum DeliveryMode {
    /// Fixed: Deliver to all processors in destination.
    Fixed = 0b000,
    /// Lowest Priority: Deliver to lowest priority processor.
    LowestPriority = 0b001,
    /// SMI: System Management Interrupt.
    Smi = 0b010,
    /// NMI: Non-Maskable Interrupt.
    Nmi = 0b100,
    /// INIT: Assert INIT signal.
    Init = 0b101,
    /// ExtINT: External interrupt (8259 compatible).
    ExtInt = 0b111,
}

/// Interrupt polarity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Polarity {
    /// Active high.
    ActiveHigh = 0,
    /// Active low.
    ActiveLow = 1,
}

/// Interrupt trigger mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TriggerMode {
    /// Edge triggered.
    Edge = 0,
    /// Level triggered.
    Level = 1,
}

/// Redirection Table Entry configuration.
#[derive(Debug, Clone, Copy)]
pub struct RedirectionEntry {
    /// Interrupt vector (32-255).
    pub vector: u8,
    /// Delivery mode.
    pub delivery_mode: DeliveryMode,
    /// Destination mode (false = Physical, true = Logical).
    pub logical_destination: bool,
    /// Interrupt polarity.
    pub polarity: Polarity,
    /// Trigger mode.
    pub trigger_mode: TriggerMode,
    /// Masked (disabled).
    pub masked: bool,
    /// Destination APIC ID.
    pub destination: u8,
}

impl RedirectionEntry {
    /// Create a new redirection entry with default values.
    pub fn new(vector: u8, destination: u8) -> Self {
        RedirectionEntry {
            vector,
            delivery_mode: DeliveryMode::Fixed,
            logical_destination: false,
            polarity: Polarity::ActiveHigh,
            trigger_mode: TriggerMode::Edge,
            masked: false,
            destination,
        }
    }
    
    /// Convert to 64-bit register value.
    fn to_u64(&self) -> u64 {
        let mut value: u64 = self.vector as u64;
        value |= (self.delivery_mode as u64) << 8;
        value |= (self.logical_destination as u64) << 11;
        value |= (self.polarity as u64) << 13;
        value |= (self.trigger_mode as u64) << 15;
        value |= (self.masked as u64) << 16;
        value |= (self.destination as u64) << 56;
        value
    }
    
    /// Create from 64-bit register value.
    fn from_u64(value: u64) -> Self {
        RedirectionEntry {
            vector: (value & 0xFF) as u8,
            delivery_mode: match (value >> 8) & 0x7 {
                0b000 => DeliveryMode::Fixed,
                0b001 => DeliveryMode::LowestPriority,
                0b010 => DeliveryMode::Smi,
                0b100 => DeliveryMode::Nmi,
                0b101 => DeliveryMode::Init,
                0b111 => DeliveryMode::ExtInt,
                _ => DeliveryMode::Fixed,
            },
            logical_destination: (value >> 11) & 1 != 0,
            polarity: if (value >> 13) & 1 != 0 {
                Polarity::ActiveLow
            } else {
                Polarity::ActiveHigh
            },
            trigger_mode: if (value >> 15) & 1 != 0 {
                TriggerMode::Level
            } else {
                TriggerMode::Edge
            },
            masked: (value >> 16) & 1 != 0,
            destination: (value >> 56) as u8,
        }
    }
}

/// I/O APIC driver.
pub struct IoApic {
    /// I/O APIC ID.
    id: u8,
    /// Base physical address.
    base_phys: u64,
    /// Base virtual address.
    base_virt: u64,
    /// Global System Interrupt base.
    gsi_base: u32,
    /// Number of redirection entries.
    max_entries: u8,
}

impl IoApic {
    /// Create a new I/O APIC instance.
    ///
    /// # Arguments
    ///
    /// - `id`: I/O APIC ID from ACPI MADT.
    /// - `base_phys`: Physical base address from ACPI MADT.
    /// - `gsi_base`: Global System Interrupt base from ACPI MADT.
    ///
    /// # Safety
    ///
    /// The physical address must be valid and the memory must be mapped.
    pub unsafe fn new(id: u8, base_phys: u64, gsi_base: u32) -> Self {
        let offset = PHYS_MEM_OFFSET.load(Ordering::SeqCst);
        let base_virt = base_phys + offset;
        
        let mut ioapic = IoApic {
            id,
            base_phys,
            base_virt,
            gsi_base,
            max_entries: 0,
        };
        
        // Read version register to get max redirection entries.
        let version = ioapic.read_internal(internal::IOAPICVER);
        ioapic.max_entries = ((version >> 16) & 0xFF) as u8 + 1;
        
        crate::serial_println!(
            "[IOAPIC] ID={}, base={:#x}, GSI base={}, entries={}",
            id, base_phys, gsi_base, ioapic.max_entries
        );
        
        ioapic
    }
    
    /// Create with default address (for systems without ACPI).
    pub unsafe fn new_default() -> Self {
        unsafe { Self::new(0, IOAPIC_BASE_DEFAULT, 0) }
    }
    
    /// Read from IOREGSEL/IOWIN registers.
    fn read_internal(&self, reg: u8) -> u32 {
        unsafe {
            let sel = (self.base_virt + regs::IOREGSEL as u64) as *mut u32;
            let win = (self.base_virt + regs::IOWIN as u64) as *const u32;
            write_volatile(sel, reg as u32);
            read_volatile(win)
        }
    }
    
    /// Write to IOREGSEL/IOWIN registers.
    fn write_internal(&self, reg: u8, value: u32) {
        unsafe {
            let sel = (self.base_virt + regs::IOREGSEL as u64) as *mut u32;
            let win = (self.base_virt + regs::IOWIN as u64) as *mut u32;
            write_volatile(sel, reg as u32);
            write_volatile(win, value);
        }
    }
    
    /// Get the I/O APIC ID.
    pub fn id(&self) -> u8 {
        self.id
    }
    
    /// Get the GSI base.
    pub fn gsi_base(&self) -> u32 {
        self.gsi_base
    }
    
    /// Get the number of redirection entries.
    pub fn max_entries(&self) -> u8 {
        self.max_entries
    }
    
    /// Check if this I/O APIC handles a given GSI.
    pub fn handles_gsi(&self, gsi: u32) -> bool {
        gsi >= self.gsi_base && gsi < self.gsi_base + self.max_entries as u32
    }
    
    /// Read a redirection table entry.
    pub fn read_entry(&self, index: u8) -> RedirectionEntry {
        assert!(index < self.max_entries, "Invalid IOREDTBL index");
        
        let reg_low = internal::IOREDTBL_BASE + index * 2;
        let reg_high = reg_low + 1;
        
        let low = self.read_internal(reg_low);
        let high = self.read_internal(reg_high);
        
        RedirectionEntry::from_u64((high as u64) << 32 | low as u64)
    }
    
    /// Write a redirection table entry.
    pub fn write_entry(&self, index: u8, entry: &RedirectionEntry) {
        assert!(index < self.max_entries, "Invalid IOREDTBL index");
        
        let reg_low = internal::IOREDTBL_BASE + index * 2;
        let reg_high = reg_low + 1;
        
        let value = entry.to_u64();
        
        // Write high first, then low (to avoid spurious interrupts).
        self.write_internal(reg_high, (value >> 32) as u32);
        self.write_internal(reg_low, value as u32);
    }
    
    /// Set up an IRQ redirection.
    ///
    /// # Arguments
    ///
    /// - `irq`: IRQ number relative to this I/O APIC's GSI base.
    /// - `vector`: Interrupt vector (32-255).
    /// - `dest_apic_id`: Destination Local APIC ID.
    pub fn set_irq(&self, irq: u8, vector: u8, dest_apic_id: u8) {
        let entry = RedirectionEntry::new(vector, dest_apic_id);
        self.write_entry(irq, &entry);
    }
    
    /// Mask (disable) an IRQ.
    pub fn mask_irq(&self, irq: u8) {
        let mut entry = self.read_entry(irq);
        entry.masked = true;
        self.write_entry(irq, &entry);
    }
    
    /// Unmask (enable) an IRQ.
    pub fn unmask_irq(&self, irq: u8) {
        let mut entry = self.read_entry(irq);
        entry.masked = false;
        self.write_entry(irq, &entry);
    }
    
    /// Mask all IRQs.
    pub fn mask_all(&self) {
        for i in 0..self.max_entries {
            self.mask_irq(i);
        }
    }
}

/// I/O APIC manager for systems with multiple I/O APICs.
pub struct IoApicManager {
    ioapics: Vec<IoApic>,
}

impl IoApicManager {
    /// Create a new I/O APIC manager.
    pub const fn new() -> Self {
        IoApicManager {
            ioapics: Vec::new(),
        }
    }
    
    /// Add an I/O APIC.
    pub fn add(&mut self, ioapic: IoApic) {
        self.ioapics.push(ioapic);
    }
    
    /// Find the I/O APIC that handles a given GSI.
    pub fn find_by_gsi(&self, gsi: u32) -> Option<&IoApic> {
        self.ioapics.iter().find(|ioapic| ioapic.handles_gsi(gsi))
    }
    
    /// Find the I/O APIC that handles a given GSI (mutable).
    pub fn find_by_gsi_mut(&mut self, gsi: u32) -> Option<&mut IoApic> {
        self.ioapics.iter_mut().find(|ioapic| ioapic.handles_gsi(gsi))
    }
    
    /// Set up a GSI redirection.
    pub fn set_gsi(&mut self, gsi: u32, vector: u8, dest_apic_id: u8) -> bool {
        if let Some(ioapic) = self.find_by_gsi_mut(gsi) {
            let irq = (gsi - ioapic.gsi_base()) as u8;
            ioapic.set_irq(irq, vector, dest_apic_id);
            true
        } else {
            false
        }
    }
    
    /// Mask a GSI.
    pub fn mask_gsi(&mut self, gsi: u32) -> bool {
        if let Some(ioapic) = self.find_by_gsi_mut(gsi) {
            let irq = (gsi - ioapic.gsi_base()) as u8;
            ioapic.mask_irq(irq);
            true
        } else {
            false
        }
    }
    
    /// Unmask a GSI.
    pub fn unmask_gsi(&mut self, gsi: u32) -> bool {
        if let Some(ioapic) = self.find_by_gsi_mut(gsi) {
            let irq = (gsi - ioapic.gsi_base()) as u8;
            ioapic.unmask_irq(irq);
            true
        } else {
            false
        }
    }
}

/// Global I/O APIC manager.
static IO_APIC_MANAGER: Mutex<IoApicManager> = Mutex::new(IoApicManager::new());

/// Set the physical memory offset.
pub fn set_physical_memory_offset(offset: u64) {
    PHYS_MEM_OFFSET.store(offset, Ordering::SeqCst);
}

/// Initialize the I/O APIC subsystem with a default I/O APIC.
///
/// This is used when ACPI is not available or parsing fails.
///
/// # Safety
///
/// Physical memory offset must be set first.
pub unsafe fn init_default() {
    let ioapic = unsafe { IoApic::new_default() };
    
    // Mask all interrupts initially.
    ioapic.mask_all();
    
    let mut manager = IO_APIC_MANAGER.lock();
    manager.add(ioapic);
    
    crate::serial_println!("[IOAPIC] Initialized with default settings");
}

/// Add an I/O APIC from ACPI MADT.
///
/// # Safety
///
/// Physical memory offset must be set first.
/// The address must be valid.
pub unsafe fn add_from_acpi(id: u8, base_addr: u64, gsi_base: u32) {
    let ioapic = unsafe { IoApic::new(id, base_addr, gsi_base) };
    ioapic.mask_all();
    
    let mut manager = IO_APIC_MANAGER.lock();
    manager.add(ioapic);
}

/// Set up a GSI redirection.
pub fn set_gsi(gsi: u32, vector: u8, dest_apic_id: u8) -> bool {
    IO_APIC_MANAGER.lock().set_gsi(gsi, vector, dest_apic_id)
}

/// Mask a GSI.
pub fn mask_gsi(gsi: u32) -> bool {
    IO_APIC_MANAGER.lock().mask_gsi(gsi)
}

/// Unmask a GSI.
pub fn unmask_gsi(gsi: u32) -> bool {
    IO_APIC_MANAGER.lock().unmask_gsi(gsi)
}
