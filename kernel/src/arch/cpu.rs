//! CPU feature detection and information.
//!
//! This module provides CPU feature detection using CPUID.

use core::arch::x86_64::__cpuid;

/// CPU vendor string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpuVendor {
    Intel,
    Amd,
    Unknown,
}

/// Get the CPU vendor.
pub fn vendor() -> CpuVendor {
    let cpuid = unsafe { __cpuid(0) };
    let vendor = [
        cpuid.ebx.to_le_bytes(),
        cpuid.edx.to_le_bytes(),
        cpuid.ecx.to_le_bytes(),
    ];
    let vendor_bytes: [u8; 12] = unsafe { core::mem::transmute(vendor) };
    
    match &vendor_bytes {
        b"GenuineIntel" => CpuVendor::Intel,
        b"AuthenticAMD" => CpuVendor::Amd,
        _ => CpuVendor::Unknown,
    }
}

/// Check if the CPU supports SSE.
pub fn has_sse() -> bool {
    let cpuid = unsafe { __cpuid(1) };
    (cpuid.edx & (1 << 25)) != 0
}

/// Check if the CPU supports SSE2.
pub fn has_sse2() -> bool {
    let cpuid = unsafe { __cpuid(1) };
    (cpuid.edx & (1 << 26)) != 0
}

/// Check if the CPU supports SSE3.
pub fn has_sse3() -> bool {
    let cpuid = unsafe { __cpuid(1) };
    (cpuid.ecx & 1) != 0
}

/// Check if the CPU supports SSE4.1.
pub fn has_sse4_1() -> bool {
    let cpuid = unsafe { __cpuid(1) };
    (cpuid.ecx & (1 << 19)) != 0
}

/// Check if the CPU supports SSE4.2.
pub fn has_sse4_2() -> bool {
    let cpuid = unsafe { __cpuid(1) };
    (cpuid.ecx & (1 << 20)) != 0
}

/// Check if the CPU supports AVX.
pub fn has_avx() -> bool {
    let cpuid = unsafe { __cpuid(1) };
    (cpuid.ecx & (1 << 28)) != 0
}

/// Check if the CPU supports AVX2.
pub fn has_avx2() -> bool {
    let cpuid = unsafe { __cpuid(7) };
    (cpuid.ebx & (1 << 5)) != 0
}

/// Check if the CPU supports FSGSBASE instructions.
pub fn has_fsgsbase() -> bool {
    let cpuid = unsafe { __cpuid(7) };
    (cpuid.ebx & 1) != 0
}

/// Check if the CPU supports XSAVE.
pub fn has_xsave() -> bool {
    let cpuid = unsafe { __cpuid(1) };
    (cpuid.ecx & (1 << 26)) != 0
}

/// Check if the CPU supports x2APIC.
pub fn has_x2apic() -> bool {
    let cpuid = unsafe { __cpuid(1) };
    (cpuid.ecx & (1 << 21)) != 0
}

/// Check if the CPU supports TSC deadline mode.
pub fn has_tsc_deadline() -> bool {
    let cpuid = unsafe { __cpuid(1) };
    (cpuid.ecx & (1 << 24)) != 0
}

/// Check if the CPU supports invariant TSC.
pub fn has_invariant_tsc() -> bool {
    // Check if extended CPUID is available
    let max_ext = unsafe { __cpuid(0x80000000) };
    if max_ext.eax < 0x80000007 {
        return false;
    }
    
    let cpuid = unsafe { __cpuid(0x80000007) };
    (cpuid.edx & (1 << 8)) != 0
}

/// Check if the CPU supports 1GB pages.
pub fn has_1gb_pages() -> bool {
    let max_ext = unsafe { __cpuid(0x80000000) };
    if max_ext.eax < 0x80000001 {
        return false;
    }
    
    let cpuid = unsafe { __cpuid(0x80000001) };
    (cpuid.edx & (1 << 26)) != 0
}

/// Check if the CPU supports NX bit.
pub fn has_nx_bit() -> bool {
    let max_ext = unsafe { __cpuid(0x80000000) };
    if max_ext.eax < 0x80000001 {
        return false;
    }
    
    let cpuid = unsafe { __cpuid(0x80000001) };
    (cpuid.edx & (1 << 20)) != 0
}

/// Check if the CPU supports SYSCALL/SYSRET.
pub fn has_syscall() -> bool {
    let max_ext = unsafe { __cpuid(0x80000000) };
    if max_ext.eax < 0x80000001 {
        return false;
    }
    
    let cpuid = unsafe { __cpuid(0x80000001) };
    (cpuid.edx & (1 << 11)) != 0
}

/// Get the number of physical address bits.
pub fn physical_address_bits() -> u8 {
    let max_ext = unsafe { __cpuid(0x80000000) };
    if max_ext.eax < 0x80000008 {
        return 36; // Default for older CPUs
    }
    
    let cpuid = unsafe { __cpuid(0x80000008) };
    (cpuid.eax & 0xFF) as u8
}

/// Get the number of virtual address bits.
pub fn virtual_address_bits() -> u8 {
    let max_ext = unsafe { __cpuid(0x80000000) };
    if max_ext.eax < 0x80000008 {
        return 48; // Default for x86_64
    }
    
    let cpuid = unsafe { __cpuid(0x80000008) };
    ((cpuid.eax >> 8) & 0xFF) as u8
}

/// CPU brand string.
pub fn brand_string() -> [u8; 48] {
    let mut brand = [0u8; 48];
    
    let max_ext = unsafe { __cpuid(0x80000000) };
    if max_ext.eax < 0x80000004 {
        return brand;
    }
    
    for i in 0..3 {
        let cpuid = unsafe { __cpuid(0x80000002 + i) };
        let offset = i as usize * 16;
        brand[offset..offset + 4].copy_from_slice(&cpuid.eax.to_le_bytes());
        brand[offset + 4..offset + 8].copy_from_slice(&cpuid.ebx.to_le_bytes());
        brand[offset + 8..offset + 12].copy_from_slice(&cpuid.ecx.to_le_bytes());
        brand[offset + 12..offset + 16].copy_from_slice(&cpuid.edx.to_le_bytes());
    }
    
    brand
}
