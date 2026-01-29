//! Hardware Detection Module
//!
//! Automatic detection of hardware components.

use alloc::vec::Vec;
use alloc::string::String;
use super::{HardwareDevice, DeviceType, DeviceStatus, DeviceResource};

/// CPU information
#[derive(Debug, Clone)]
pub struct CpuInfo {
    /// Vendor string (e.g., "GenuineIntel", "AuthenticAMD")
    pub vendor: String,
    /// Brand string
    pub brand: String,
    /// Family
    pub family: u8,
    /// Model
    pub model: u8,
    /// Stepping
    pub stepping: u8,
    /// Number of physical cores
    pub cores: u8,
    /// Number of logical processors
    pub threads: u8,
    /// CPU features
    pub features: CpuFeatures,
}

/// CPU feature flags
#[derive(Debug, Clone, Default)]
pub struct CpuFeatures {
    /// SSE support
    pub sse: bool,
    /// SSE2 support
    pub sse2: bool,
    /// SSE3 support
    pub sse3: bool,
    /// SSE4.1 support
    pub sse4_1: bool,
    /// SSE4.2 support
    pub sse4_2: bool,
    /// AVX support
    pub avx: bool,
    /// AVX2 support
    pub avx2: bool,
    /// AVX-512 support
    pub avx512: bool,
    /// AES-NI support
    pub aesni: bool,
    /// RDRAND support
    pub rdrand: bool,
    /// x2APIC support
    pub x2apic: bool,
    /// TSC deadline timer
    pub tsc_deadline: bool,
    /// 1GB page support
    pub huge_pages_1gb: bool,
    /// PCID support
    pub pcid: bool,
    /// SMAP support
    pub smap: bool,
    /// SMEP support
    pub smep: bool,
}

/// Memory information
#[derive(Debug, Clone)]
pub struct MemoryInfo {
    /// Total physical memory in bytes
    pub total: u64,
    /// Available memory in bytes
    pub available: u64,
    /// Memory regions
    pub regions: Vec<MemoryRegion>,
}

/// Memory region from BIOS/UEFI memory map
#[derive(Debug, Clone)]
pub struct MemoryRegion {
    /// Physical start address
    pub base: u64,
    /// Length in bytes
    pub length: u64,
    /// Region type
    pub region_type: MemoryRegionType,
}

/// Memory region types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryRegionType {
    /// Usable RAM
    Usable,
    /// Reserved by system
    Reserved,
    /// ACPI reclaimable
    AcpiReclaimable,
    /// ACPI NVS
    AcpiNvs,
    /// Bad memory
    BadMemory,
    /// Bootloader reclaimable
    BootloaderReclaimable,
    /// Kernel and modules
    KernelAndModules,
    /// Framebuffer
    Framebuffer,
}

/// Detect CPU information using CPUID
pub fn detect_cpu() -> CpuInfo {
    let mut info = CpuInfo {
        vendor: String::new(),
        brand: String::new(),
        family: 0,
        model: 0,
        stepping: 0,
        cores: 1,
        threads: 1,
        features: CpuFeatures::default(),
    };

    #[cfg(target_arch = "x86_64")]
    unsafe {
        // Get vendor string (CPUID function 0)
        let cpuid_result = core::arch::x86_64::__cpuid(0);
        let vendor_bytes: [u8; 12] = [
            (cpuid_result.ebx & 0xFF) as u8,
            ((cpuid_result.ebx >> 8) & 0xFF) as u8,
            ((cpuid_result.ebx >> 16) & 0xFF) as u8,
            ((cpuid_result.ebx >> 24) & 0xFF) as u8,
            (cpuid_result.edx & 0xFF) as u8,
            ((cpuid_result.edx >> 8) & 0xFF) as u8,
            ((cpuid_result.edx >> 16) & 0xFF) as u8,
            ((cpuid_result.edx >> 24) & 0xFF) as u8,
            (cpuid_result.ecx & 0xFF) as u8,
            ((cpuid_result.ecx >> 8) & 0xFF) as u8,
            ((cpuid_result.ecx >> 16) & 0xFF) as u8,
            ((cpuid_result.ecx >> 24) & 0xFF) as u8,
        ];
        if let Ok(s) = core::str::from_utf8(&vendor_bytes) {
            info.vendor = String::from(s);
        }

        // Get family, model, stepping (CPUID function 1)
        let cpuid_result = core::arch::x86_64::__cpuid(1);
        info.stepping = (cpuid_result.eax & 0xF) as u8;
        info.model = ((cpuid_result.eax >> 4) & 0xF) as u8;
        info.family = ((cpuid_result.eax >> 8) & 0xF) as u8;

        // Extended model/family for newer CPUs
        if info.family == 0xF {
            info.family += ((cpuid_result.eax >> 20) & 0xFF) as u8;
        }
        if info.family >= 6 {
            info.model += (((cpuid_result.eax >> 16) & 0xF) as u8) << 4;
        }

        // Check feature flags from EDX
        let edx = cpuid_result.edx;
        info.features.sse = (edx & (1 << 25)) != 0;
        info.features.sse2 = (edx & (1 << 26)) != 0;

        // Check feature flags from ECX
        let ecx = cpuid_result.ecx;
        info.features.sse3 = (ecx & (1 << 0)) != 0;
        info.features.sse4_1 = (ecx & (1 << 19)) != 0;
        info.features.sse4_2 = (ecx & (1 << 20)) != 0;
        info.features.aesni = (ecx & (1 << 25)) != 0;
        info.features.avx = (ecx & (1 << 28)) != 0;
        info.features.rdrand = (ecx & (1 << 30)) != 0;
        info.features.x2apic = (ecx & (1 << 21)) != 0;
        info.features.tsc_deadline = (ecx & (1 << 24)) != 0;
        info.features.pcid = (ecx & (1 << 17)) != 0;

        // Extended features (CPUID function 7)
        let cpuid_result = core::arch::x86_64::__cpuid_count(7, 0);
        let ebx = cpuid_result.ebx;
        info.features.avx2 = (ebx & (1 << 5)) != 0;
        info.features.avx512 = (ebx & (1 << 16)) != 0;  // AVX-512F
        info.features.smap = (ebx & (1 << 20)) != 0;
        info.features.smep = (ebx & (1 << 7)) != 0;

        // Extended features for huge pages (CPUID function 0x80000001)
        let cpuid_result = core::arch::x86_64::__cpuid(0x80000001);
        info.features.huge_pages_1gb = (cpuid_result.edx & (1 << 26)) != 0;

        // Get brand string (CPUID functions 0x80000002-0x80000004)
        let mut brand_bytes = [0u8; 48];
        for i in 0..3 {
            let cpuid_result = core::arch::x86_64::__cpuid(0x80000002 + i);
            let offset = i as usize * 16;
            brand_bytes[offset..offset + 4].copy_from_slice(&cpuid_result.eax.to_le_bytes());
            brand_bytes[offset + 4..offset + 8].copy_from_slice(&cpuid_result.ebx.to_le_bytes());
            brand_bytes[offset + 8..offset + 12].copy_from_slice(&cpuid_result.ecx.to_le_bytes());
            brand_bytes[offset + 12..offset + 16].copy_from_slice(&cpuid_result.edx.to_le_bytes());
        }
        if let Ok(s) = core::str::from_utf8(&brand_bytes) {
            info.brand = String::from(s.trim_end_matches('\0').trim());
        }

        // Core count (CPUID function 4 for Intel, function 0x80000008 for AMD)
        if info.vendor.contains("Intel") {
            let cpuid_result = core::arch::x86_64::__cpuid_count(4, 0);
            info.cores = (((cpuid_result.eax >> 26) & 0x3F) + 1) as u8;
        } else if info.vendor.contains("AMD") {
            let cpuid_result = core::arch::x86_64::__cpuid(0x80000008);
            info.cores = ((cpuid_result.ecx & 0xFF) + 1) as u8;
        }

        // Thread count from CPUID function 1
        let cpuid_result = core::arch::x86_64::__cpuid(1);
        info.threads = ((cpuid_result.ebx >> 16) & 0xFF) as u8;
    }

    info
}

/// Detect installed RAM from firmware memory map
pub fn detect_memory() -> MemoryInfo {
    // In a real implementation, this would parse the memory map from
    // bootloader (Limine, GRUB, UEFI directly, etc.)
    MemoryInfo {
        total: 0,
        available: 0,
        regions: Vec::new(),
    }
}

/// Perform full hardware detection scan
pub fn detect_all() -> HardwareDetectionResult {
    HardwareDetectionResult {
        cpu: detect_cpu(),
        memory: detect_memory(),
        devices: Vec::new(),
    }
}

/// Result of hardware detection
pub struct HardwareDetectionResult {
    /// CPU information
    pub cpu: CpuInfo,
    /// Memory information
    pub memory: MemoryInfo,
    /// Discovered devices
    pub devices: Vec<HardwareDevice>,
}
