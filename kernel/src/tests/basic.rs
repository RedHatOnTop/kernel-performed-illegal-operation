//! Basic kernel functionality tests
//!
//! Tests core features that must be verified in Phase 0.

use crate::kernel_test;

kernel_test!(test_breakpoint_exception, {
    // Breakpoint exception should not cause a panic
    x86_64::instructions::interrupts::int3();
});

kernel_test!(test_heap_allocation, {
    use alloc::boxed::Box;
    use alloc::vec::Vec;

    // Simple heap allocation
    let heap_value = Box::new(42);
    assert_eq!(*heap_value, 42);

    // Vector allocation
    let mut vec = Vec::new();
    for i in 0..100 {
        vec.push(i);
    }
    assert_eq!(vec.len(), 100);
    assert_eq!(vec[50], 50);
});

kernel_test!(test_large_allocation, {
    use alloc::vec::Vec;

    // Large allocation test
    let n: u64 = 1000;
    let mut vec = Vec::new();
    for i in 0..n {
        vec.push(i);
    }
    assert_eq!(vec.iter().sum::<u64>(), (n - 1) * n / 2);
});

kernel_test!(test_reallocation, {
    use alloc::vec::Vec;

    // Reallocation test (vector capacity growth)
    let mut vec = Vec::new();
    for i in 0..1000 {
        vec.push(i);
    }

    // The vector should have been reallocated multiple times
    assert_eq!(vec.len(), 1000);
    assert!(vec.capacity() >= 1000);
});

kernel_test!(test_allocation_deallocation, {
    use alloc::boxed::Box;

    // Test that allocation and deallocation work correctly
    for _ in 0..1000 {
        let _val = Box::new([0u8; 100]);
    }
    // Should complete without memory leaks
});
