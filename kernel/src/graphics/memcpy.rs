//! Memory Copy Optimizations
//!
//! High-performance memory operations for framebuffer manipulation.
//! Uses SIMD instructions when available for maximum throughput.

use core::arch::x86_64::*;

/// Fast memory copy using SIMD when possible
///
/// # Safety
/// Both `src` and `dst` must be valid pointers and the memory regions
/// must not overlap (or use memmove instead).
#[inline]
pub unsafe fn fast_copy(dst: *mut u8, src: *const u8, len: usize) {
    if len == 0 {
        return;
    }

    // For small copies, use simple byte copy
    if len < 64 {
        unsafe {
            core::ptr::copy_nonoverlapping(src, dst, len);
        }
        return;
    }

    // Use SIMD for larger copies
    unsafe {
        fast_copy_simd(dst, src, len);
    }
}

/// SIMD-optimized memory copy
#[inline]
#[target_feature(enable = "sse2")]
unsafe fn fast_copy_simd(mut dst: *mut u8, mut src: *const u8, mut len: usize) {
    // Handle unaligned prefix
    let align = dst as usize & 15;
    if align != 0 {
        let prefix_len = (16 - align).min(len);
        unsafe {
            core::ptr::copy_nonoverlapping(src, dst, prefix_len);
        }
        dst = unsafe { dst.add(prefix_len) };
        src = unsafe { src.add(prefix_len) };
        len -= prefix_len;
    }

    // Main SIMD loop - copy 64 bytes at a time
    while len >= 64 {
        unsafe {
            let xmm0 = _mm_loadu_si128(src as *const __m128i);
            let xmm1 = _mm_loadu_si128(src.add(16) as *const __m128i);
            let xmm2 = _mm_loadu_si128(src.add(32) as *const __m128i);
            let xmm3 = _mm_loadu_si128(src.add(48) as *const __m128i);

            _mm_store_si128(dst as *mut __m128i, xmm0);
            _mm_store_si128(dst.add(16) as *mut __m128i, xmm1);
            _mm_store_si128(dst.add(32) as *mut __m128i, xmm2);
            _mm_store_si128(dst.add(48) as *mut __m128i, xmm3);

            dst = dst.add(64);
            src = src.add(64);
        }
        len -= 64;
    }

    // Handle remaining bytes
    if len >= 16 {
        while len >= 16 {
            unsafe {
                let xmm = _mm_loadu_si128(src as *const __m128i);
                _mm_store_si128(dst as *mut __m128i, xmm);
                dst = dst.add(16);
                src = src.add(16);
            }
            len -= 16;
        }
    }

    // Copy remaining bytes
    if len > 0 {
        unsafe {
            core::ptr::copy_nonoverlapping(src, dst, len);
        }
    }
}

/// Fast memory set using SIMD
///
/// # Safety
/// `dst` must be valid and have at least `len` bytes available.
#[inline]
pub unsafe fn fast_set(dst: *mut u8, value: u8, len: usize) {
    if len == 0 {
        return;
    }

    if len < 64 {
        unsafe {
            core::ptr::write_bytes(dst, value, len);
        }
        return;
    }

    unsafe {
        fast_set_simd(dst, value, len);
    }
}

/// SIMD-optimized memory set
#[inline]
#[target_feature(enable = "sse2")]
unsafe fn fast_set_simd(mut dst: *mut u8, value: u8, mut len: usize) {
    // Create a vector filled with the value
    let v = unsafe { _mm_set1_epi8(value as i8) };

    // Handle unaligned prefix
    let align = dst as usize & 15;
    if align != 0 {
        let prefix_len = (16 - align).min(len);
        unsafe {
            core::ptr::write_bytes(dst, value, prefix_len);
        }
        dst = unsafe { dst.add(prefix_len) };
        len -= prefix_len;
    }

    // Main SIMD loop
    while len >= 64 {
        unsafe {
            _mm_store_si128(dst as *mut __m128i, v);
            _mm_store_si128(dst.add(16) as *mut __m128i, v);
            _mm_store_si128(dst.add(32) as *mut __m128i, v);
            _mm_store_si128(dst.add(48) as *mut __m128i, v);
            dst = dst.add(64);
        }
        len -= 64;
    }

    while len >= 16 {
        unsafe {
            _mm_store_si128(dst as *mut __m128i, v);
            dst = dst.add(16);
        }
        len -= 16;
    }

    if len > 0 {
        unsafe {
            core::ptr::write_bytes(dst, value, len);
        }
    }
}

/// Fast 32-bit value set (for pixel filling)
///
/// # Safety
/// `dst` must be valid and properly aligned for u32, with at least `count` u32s available.
#[inline]
pub unsafe fn fast_set32(dst: *mut u32, value: u32, count: usize) {
    if count == 0 {
        return;
    }

    if count < 16 {
        for i in 0..count {
            unsafe {
                *dst.add(i) = value;
            }
        }
        return;
    }

    unsafe {
        fast_set32_simd(dst, value, count);
    }
}

/// SIMD-optimized 32-bit set
#[inline]
#[target_feature(enable = "sse2")]
unsafe fn fast_set32_simd(mut dst: *mut u32, value: u32, mut count: usize) {
    let v = unsafe { _mm_set1_epi32(value as i32) };

    // Handle alignment
    let align = (dst as usize & 15) / 4;
    if align != 0 {
        let prefix = (4 - align).min(count);
        for i in 0..prefix {
            unsafe {
                *dst.add(i) = value;
            }
        }
        dst = unsafe { dst.add(prefix) };
        count -= prefix;
    }

    // Main loop - 16 pixels at a time
    while count >= 16 {
        unsafe {
            _mm_store_si128(dst as *mut __m128i, v);
            _mm_store_si128(dst.add(4) as *mut __m128i, v);
            _mm_store_si128(dst.add(8) as *mut __m128i, v);
            _mm_store_si128(dst.add(12) as *mut __m128i, v);
            dst = dst.add(16);
        }
        count -= 16;
    }

    while count >= 4 {
        unsafe {
            _mm_store_si128(dst as *mut __m128i, v);
            dst = dst.add(4);
        }
        count -= 4;
    }

    for i in 0..count {
        unsafe {
            *dst.add(i) = value;
        }
    }
}

/// Copy a rectangular region from one buffer to another
///
/// # Safety
/// Both buffers must be valid and have sufficient size.
pub unsafe fn copy_rect(
    dst: *mut u8,
    dst_stride: usize,
    src: *const u8,
    src_stride: usize,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    bpp: usize,
) {
    let row_bytes = width * bpp;

    for row in 0..height {
        let dst_offset = (y + row) * dst_stride * bpp + x * bpp;
        let src_offset = (y + row) * src_stride * bpp + x * bpp;

        unsafe {
            fast_copy(dst.add(dst_offset), src.add(src_offset), row_bytes);
        }
    }
}

/// Fill a rectangular region with a color
///
/// # Safety
/// Buffer must be valid and have sufficient size.
pub unsafe fn fill_rect_fast(
    dst: *mut u8,
    stride: usize,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    color: u32, // BGRA format
    bpp: usize,
) {
    if bpp == 4 {
        // Optimized path for 32-bit pixels
        for row in 0..height {
            let row_ptr = unsafe { dst.add((y + row) * stride * bpp + x * bpp) as *mut u32 };
            unsafe {
                fast_set32(row_ptr, color, width);
            }
        }
    } else {
        // Generic path
        let b = (color & 0xFF) as u8;
        let g = ((color >> 8) & 0xFF) as u8;
        let r = ((color >> 16) & 0xFF) as u8;

        for row in 0..height {
            for col in 0..width {
                let offset = (y + row) * stride * bpp + (x + col) * bpp;
                unsafe {
                    *dst.add(offset) = b;
                    *dst.add(offset + 1) = g;
                    *dst.add(offset + 2) = r;
                }
            }
        }
    }
}
