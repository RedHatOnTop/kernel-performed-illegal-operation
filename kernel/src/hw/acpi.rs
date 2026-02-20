//! ACPI (Advanced Configuration and Power Interface) Support
//!
//! Parses ACPI tables for hardware configuration and power management.

use alloc::string::String;
use alloc::vec::Vec;
use core::ptr;

/// ACPI Root System Description Pointer (RSDP)
#[repr(C, packed)]
pub struct Rsdp {
    /// "RSD PTR " signature
    pub signature: [u8; 8],
    /// Checksum for first 20 bytes
    pub checksum: u8,
    /// OEM ID string
    pub oem_id: [u8; 6],
    /// Revision (0 = ACPI 1.0, 2 = ACPI 2.0+)
    pub revision: u8,
    /// Physical address of RSDT (32-bit)
    pub rsdt_address: u32,
    // ACPI 2.0+ fields below
    /// Length of table
    pub length: u32,
    /// Physical address of XSDT (64-bit)
    pub xsdt_address: u64,
    /// Extended checksum
    pub extended_checksum: u8,
    /// Reserved
    pub reserved: [u8; 3],
}

/// Standard ACPI table header
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct AcpiTableHeader {
    /// 4-byte signature
    pub signature: [u8; 4],
    /// Table length including header
    pub length: u32,
    /// Revision
    pub revision: u8,
    /// Checksum
    pub checksum: u8,
    /// OEM ID
    pub oem_id: [u8; 6],
    /// OEM table ID
    pub oem_table_id: [u8; 8],
    /// OEM revision
    pub oem_revision: u32,
    /// Creator ID
    pub creator_id: u32,
    /// Creator revision
    pub creator_revision: u32,
}

/// MADT (Multiple APIC Description Table) entry header
#[repr(C, packed)]
pub struct MadtEntryHeader {
    /// Entry type
    pub entry_type: u8,
    /// Entry length
    pub length: u8,
}

/// MADT entry types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MadtEntryType {
    LocalApic = 0,
    IoApic = 1,
    InterruptSourceOverride = 2,
    NmiSource = 3,
    LocalApicNmi = 4,
    LocalApicAddressOverride = 5,
    IoSapic = 6,
    LocalSapic = 7,
    PlatformInterruptSources = 8,
    ProcessorLocalX2Apic = 9,
    LocalX2ApicNmi = 10,
    Gic = 11,
    Gicd = 12,
}

/// Local APIC entry
#[repr(C, packed)]
pub struct MadtLocalApic {
    /// Header
    pub header: MadtEntryHeader,
    /// ACPI processor ID
    pub acpi_processor_id: u8,
    /// APIC ID
    pub apic_id: u8,
    /// Flags (bit 0: enabled, bit 1: online capable)
    pub flags: u32,
}

/// I/O APIC entry
#[repr(C, packed)]
pub struct MadtIoApic {
    /// Header
    pub header: MadtEntryHeader,
    /// I/O APIC ID
    pub io_apic_id: u8,
    /// Reserved
    pub reserved: u8,
    /// I/O APIC address
    pub io_apic_address: u32,
    /// Global system interrupt base
    pub global_system_interrupt_base: u32,
}

/// Interrupt source override
#[repr(C, packed)]
pub struct MadtInterruptOverride {
    /// Header
    pub header: MadtEntryHeader,
    /// Bus (always 0 = ISA)
    pub bus: u8,
    /// Source IRQ
    pub source: u8,
    /// Global system interrupt
    pub global_system_interrupt: u32,
    /// Flags
    pub flags: u16,
}

/// ACPI table manager
pub struct AcpiTables {
    /// RSDP location
    rsdp_address: u64,
    /// ACPI revision
    revision: u8,
    /// Found tables
    tables: Vec<FoundTable>,
}

/// A found ACPI table
#[derive(Debug)]
pub struct FoundTable {
    /// Signature
    pub signature: [u8; 4],
    /// Physical address
    pub address: u64,
    /// Table length
    pub length: u32,
}

impl AcpiTables {
    /// Create empty tables
    pub const fn empty() -> Self {
        Self {
            rsdp_address: 0,
            revision: 0,
            tables: Vec::new(),
        }
    }

    /// Parse ACPI tables from RSDP address
    pub unsafe fn parse(rsdp_addr: u64, phys_mem_offset: u64) -> Result<Self, &'static str> {
        let rsdp = unsafe { &*((rsdp_addr + phys_mem_offset) as *const Rsdp) };

        // Verify signature
        if &rsdp.signature != b"RSD PTR " {
            return Err("Invalid RSDP signature");
        }

        // Verify checksum
        let checksum: u8 = (0..20)
            .map(|i| unsafe { *(((rsdp_addr + phys_mem_offset) as *const u8).add(i)) })
            .fold(0u8, |acc, b| acc.wrapping_add(b));

        if checksum != 0 {
            return Err("Invalid RSDP checksum");
        }

        let mut acpi = Self {
            rsdp_address: rsdp_addr,
            revision: rsdp.revision,
            tables: Vec::new(),
        };

        // Parse RSDT or XSDT
        if rsdp.revision >= 2 && rsdp.xsdt_address != 0 {
            unsafe { acpi.parse_xsdt(rsdp.xsdt_address, phys_mem_offset)? };
        } else {
            unsafe { acpi.parse_rsdt(rsdp.rsdt_address as u64, phys_mem_offset)? };
        }

        Ok(acpi)
    }

    /// Parse RSDT (32-bit pointers)
    unsafe fn parse_rsdt(&mut self, rsdt_addr: u64, phys_mem_offset: u64) -> Result<(), &'static str> {
        let header = unsafe { &*((rsdt_addr + phys_mem_offset) as *const AcpiTableHeader) };

        if &header.signature != b"RSDT" {
            return Err("Invalid RSDT signature");
        }

        let entry_count = (header.length as usize - core::mem::size_of::<AcpiTableHeader>()) / 4;
        let entries = (rsdt_addr + phys_mem_offset + core::mem::size_of::<AcpiTableHeader>() as u64) as *const u32;

        for i in 0..entry_count {
            let table_addr = unsafe { *entries.add(i) } as u64;
            unsafe { self.add_table(table_addr, phys_mem_offset)? };
        }

        Ok(())
    }

    /// Parse XSDT (64-bit pointers)
    unsafe fn parse_xsdt(&mut self, xsdt_addr: u64, phys_mem_offset: u64) -> Result<(), &'static str> {
        let header = unsafe { &*((xsdt_addr + phys_mem_offset) as *const AcpiTableHeader) };

        if &header.signature != b"XSDT" {
            return Err("Invalid XSDT signature");
        }

        let entry_count = (header.length as usize - core::mem::size_of::<AcpiTableHeader>()) / 8;
        let entries = (xsdt_addr + phys_mem_offset + core::mem::size_of::<AcpiTableHeader>() as u64) as *const u64;

        for i in 0..entry_count {
            let table_addr = unsafe { *entries.add(i) };
            unsafe { self.add_table(table_addr, phys_mem_offset)? };
        }

        Ok(())
    }

    /// Add a table to the list
    unsafe fn add_table(&mut self, table_addr: u64, phys_mem_offset: u64) -> Result<(), &'static str> {
        let header = unsafe { &*((table_addr + phys_mem_offset) as *const AcpiTableHeader) };

        self.tables.push(FoundTable {
            signature: header.signature,
            address: table_addr,
            length: header.length,
        });

        Ok(())
    }

    /// Find a table by signature
    pub fn find_table(&self, signature: &[u8; 4]) -> Option<&FoundTable> {
        self.tables.iter().find(|t| &t.signature == signature)
    }

    /// Get MADT (Multiple APIC Description Table)
    pub fn get_madt(&self) -> Option<&FoundTable> {
        self.find_table(b"APIC")
    }

    /// Get FADT (Fixed ACPI Description Table)
    pub fn get_fadt(&self) -> Option<&FoundTable> {
        self.find_table(b"FACP")
    }

    /// Get HPET table
    pub fn get_hpet(&self) -> Option<&FoundTable> {
        self.find_table(b"HPET")
    }

    /// Get MCFG (PCI Express memory mapped configuration)
    pub fn get_mcfg(&self) -> Option<&FoundTable> {
        self.find_table(b"MCFG")
    }

    /// Get the number of parsed tables
    pub fn table_count(&self) -> usize {
        self.tables.len()
    }
}

/// Parsed MADT information
pub struct MadtInfo {
    /// Local APIC address
    pub local_apic_address: u64,
    /// I/O APICs
    pub io_apics: Vec<IoApicInfo>,
    /// Local APICs (one per CPU)
    pub local_apics: Vec<LocalApicInfo>,
    /// Interrupt source overrides
    pub overrides: Vec<InterruptOverride>,
}

/// I/O APIC information
#[derive(Debug, Clone)]
pub struct IoApicInfo {
    pub id: u8,
    pub address: u32,
    pub gsi_base: u32,
}

/// Local APIC information
#[derive(Debug, Clone)]
pub struct LocalApicInfo {
    pub processor_id: u8,
    pub apic_id: u8,
    pub enabled: bool,
}

/// Interrupt source override
#[derive(Debug, Clone)]
pub struct InterruptOverride {
    pub source_irq: u8,
    pub global_irq: u32,
    pub polarity: u8,
    pub trigger_mode: u8,
}

impl MadtInfo {
    /// Parse MADT from table address
    pub unsafe fn parse(madt_addr: u64, phys_mem_offset: u64) -> Result<Self, &'static str> {
        let header = unsafe { &*((madt_addr + phys_mem_offset) as *const AcpiTableHeader) };

        if &header.signature != b"APIC" {
            return Err("Invalid MADT signature");
        }

        // Local APIC address is right after the header
        let local_apic_addr_ptr =
            (madt_addr + phys_mem_offset + core::mem::size_of::<AcpiTableHeader>() as u64) as *const u32;
        let local_apic_address = unsafe { *local_apic_addr_ptr } as u64;

        // Flags are after local APIC address
        let _flags = unsafe { *((local_apic_addr_ptr as u64 + 4) as *const u32) };

        let mut info = MadtInfo {
            local_apic_address,
            io_apics: Vec::new(),
            local_apics: Vec::new(),
            overrides: Vec::new(),
        };

        // Parse entries starting after header + local_apic_addr + flags
        let entries_start = madt_addr + phys_mem_offset + core::mem::size_of::<AcpiTableHeader>() as u64 + 8;
        let entries_end = madt_addr + phys_mem_offset + header.length as u64;

        let mut current = entries_start;
        while current < entries_end {
            let entry_header = unsafe { &*(current as *const MadtEntryHeader) };

            match entry_header.entry_type {
                0 => {
                    // Local APIC
                    let entry = unsafe { &*(current as *const MadtLocalApic) };
                    info.local_apics.push(LocalApicInfo {
                        processor_id: entry.acpi_processor_id,
                        apic_id: entry.apic_id,
                        enabled: (entry.flags & 1) != 0,
                    });
                }
                1 => {
                    // I/O APIC
                    let entry = unsafe { &*(current as *const MadtIoApic) };
                    info.io_apics.push(IoApicInfo {
                        id: entry.io_apic_id,
                        address: entry.io_apic_address,
                        gsi_base: entry.global_system_interrupt_base,
                    });
                }
                2 => {
                    // Interrupt Source Override
                    let entry = unsafe { &*(current as *const MadtInterruptOverride) };
                    info.overrides.push(InterruptOverride {
                        source_irq: entry.source,
                        global_irq: entry.global_system_interrupt,
                        polarity: (entry.flags & 0x3) as u8,
                        trigger_mode: ((entry.flags >> 2) & 0x3) as u8,
                    });
                }
                _ => {
                    // Skip unknown entry types
                }
            }

            current += entry_header.length as u64;
            if entry_header.length == 0 {
                break; // Prevent infinite loop
            }
        }

        Ok(info)
    }
}

/// Global ACPI tables instance
static ACPI_TABLES: spin::Once<AcpiTables> = spin::Once::new();

/// Global MADT info
static MADT_INFO: spin::Once<MadtInfo> = spin::Once::new();

/// Initialize ACPI subsystem with RSDP address from bootloader.
pub fn init_with_rsdp(rsdp_addr: u64, phys_mem_offset: u64) -> Result<(), &'static str> {
    let tables = unsafe { AcpiTables::parse(rsdp_addr, phys_mem_offset)? };

    // Try to parse MADT if available
    if let Some(madt_table) = tables.get_madt() {
        match unsafe { MadtInfo::parse(madt_table.address, phys_mem_offset) } {
            Ok(info) => {
                crate::serial_println!(
                    "[ACPI] MADT: {} local APICs, {} I/O APICs, {} overrides",
                    info.local_apics.len(),
                    info.io_apics.len(),
                    info.overrides.len()
                );
                MADT_INFO.call_once(|| info);
            }
            Err(e) => {
                crate::serial_println!("[ACPI] MADT parse failed: {}", e);
            }
        }
    }

    let table_count = tables.table_count();
    crate::serial_println!("[ACPI] Parsed {} ACPI table(s)", table_count);
    ACPI_TABLES.call_once(|| tables);
    Ok(())
}

/// Initialize ACPI subsystem (no RSDP address available).
pub fn init() -> Result<(), &'static str> {
    Err("ACPI RSDP address not provided by bootloader")
}

/// Get the number of parsed ACPI tables.
pub fn table_count() -> usize {
    ACPI_TABLES.get().map_or(0, |t| t.table_count())
}

/// Get ACPI table signatures as strings.
pub fn table_signatures() -> alloc::vec::Vec<alloc::string::String> {
    ACPI_TABLES
        .get()
        .map_or(alloc::vec::Vec::new(), |t| {
            t.tables
                .iter()
                .map(|ft| alloc::string::String::from_utf8_lossy(&ft.signature).into_owned())
                .collect()
        })
}

/// Get MADT local APIC count.
pub fn local_apic_count() -> usize {
    MADT_INFO.get().map_or(0, |m| m.local_apics.len())
}

/// Get MADT I/O APIC count.
pub fn io_apic_count() -> usize {
    MADT_INFO.get().map_or(0, |m| m.io_apics.len())
}

/// Get ACPI tables reference  
pub fn tables() -> Option<&'static AcpiTables> {
    ACPI_TABLES.get()
}
