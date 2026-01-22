//! 기본 커널 기능 테스트
//!
//! Phase 0에서 검증해야 할 핵심 기능들을 테스트합니다.

use crate::kernel_test;

kernel_test!(test_breakpoint_exception, {
    // 브레이크포인트 예외가 패닉을 일으키지 않아야 함
    x86_64::instructions::interrupts::int3();
});

kernel_test!(test_heap_allocation, {
    use alloc::boxed::Box;
    use alloc::vec::Vec;

    // 단순 힙 할당
    let heap_value = Box::new(42);
    assert_eq!(*heap_value, 42);

    // 벡터 할당
    let mut vec = Vec::new();
    for i in 0..100 {
        vec.push(i);
    }
    assert_eq!(vec.len(), 100);
    assert_eq!(vec[50], 50);
});

kernel_test!(test_large_allocation, {
    use alloc::vec::Vec;

    // 대용량 할당 테스트
    let n: u64 = 1000;
    let mut vec = Vec::new();
    for i in 0..n {
        vec.push(i);
    }
    assert_eq!(vec.iter().sum::<u64>(), (n - 1) * n / 2);
});

kernel_test!(test_reallocation, {
    use alloc::vec::Vec;

    // 재할당 테스트 (벡터 용량 증가)
    let mut vec = Vec::new();
    for i in 0..1000 {
        vec.push(i);
    }

    // 벡터가 여러 번 재할당되었을 것
    assert_eq!(vec.len(), 1000);
    assert!(vec.capacity() >= 1000);
});

kernel_test!(test_allocation_deallocation, {
    use alloc::boxed::Box;

    // 할당 후 해제가 올바르게 동작하는지 테스트
    for _ in 0..1000 {
        let _val = Box::new([0u8; 100]);
    }
    // 메모리 누수 없이 완료되어야 함
});
