//! KPIO Memory Safety Verification
//!
//! Tests and verification tools for memory safety properties.

#![no_std]
extern crate alloc;

use alloc::boxed::Box;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use spin::Mutex;

/// Safety check result
#[derive(Debug, Clone)]
pub enum SafetyResult {
    /// Check passed
    Pass,
    /// Check failed with error
    Fail(String),
}

impl SafetyResult {
    pub fn passed(&self) -> bool {
        matches!(self, SafetyResult::Pass)
    }
}

/// Safety check trait
pub trait SafetyCheck {
    fn name(&self) -> &str;
    fn run(&mut self) -> SafetyResult;
}

/// Heap allocation test
pub struct HeapAllocTest;

impl SafetyCheck for HeapAllocTest {
    fn name(&self) -> &str {
        "heap_allocation"
    }

    fn run(&mut self) -> SafetyResult {
        let mut blocks: Vec<Vec<u8>> = Vec::new();
        for i in 0..100 {
            let mut block = Vec::with_capacity(1024);
            for j in 0..1024 {
                block.push(((i + j) & 0xFF) as u8);
            }
            blocks.push(block);
        }

        for (i, block) in blocks.iter().enumerate() {
            for (j, &byte) in block.iter().enumerate() {
                if byte != ((i + j) & 0xFF) as u8 {
                    return SafetyResult::Fail(String::from("Memory corruption"));
                }
            }
        }
        SafetyResult::Pass
    }
}

/// Vec bounds test
pub struct VecBoundsTest;

impl SafetyCheck for VecBoundsTest {
    fn name(&self) -> &str {
        "vec_bounds"
    }

    fn run(&mut self) -> SafetyResult {
        let data = vec![1, 2, 3, 4, 5];

        // Valid access
        for i in 0..5 {
            if data.get(i) != Some(&(i as i32 + 1)) {
                return SafetyResult::Fail(String::from("Index mismatch"));
            }
        }

        // Out of bounds should be None
        if data.get(100).is_some() {
            return SafetyResult::Fail(String::from("get(100) should be None"));
        }

        SafetyResult::Pass
    }
}

/// Mutex test
pub struct MutexSafetyTest;

impl SafetyCheck for MutexSafetyTest {
    fn name(&self) -> &str {
        "mutex_safety"
    }

    fn run(&mut self) -> SafetyResult {
        let counter = Arc::new(Mutex::new(0i32));

        for _ in 0..100 {
            let mut guard = counter.lock();
            *guard += 1;
        }

        if *counter.lock() != 100 {
            return SafetyResult::Fail(String::from("Counter mismatch"));
        }

        SafetyResult::Pass
    }
}

/// Arc reference counting test
pub struct ArcRefCountTest;

impl SafetyCheck for ArcRefCountTest {
    fn name(&self) -> &str {
        "arc_refcount"
    }

    fn run(&mut self) -> SafetyResult {
        let data: Arc<Vec<i32>> = Arc::new(vec![1, 2, 3, 4, 5]);
        let clone1 = Arc::clone(&data);
        let clone2 = Arc::clone(&data);

        if Arc::strong_count(&data) != 3 {
            return SafetyResult::Fail(String::from("Strong count should be 3"));
        }

        drop(clone1);
        if Arc::strong_count(&data) != 2 {
            return SafetyResult::Fail(String::from("Strong count should be 2"));
        }

        drop(clone2);
        if Arc::strong_count(&data) != 1 {
            return SafetyResult::Fail(String::from("Strong count should be 1"));
        }

        SafetyResult::Pass
    }
}

/// Box allocation test
pub struct BoxAllocTest;

impl SafetyCheck for BoxAllocTest {
    fn name(&self) -> &str {
        "box_allocation"
    }

    fn run(&mut self) -> SafetyResult {
        let boxed: Box<Box<Box<i32>>> = Box::new(Box::new(Box::new(42)));
        if ***boxed != 42 {
            return SafetyResult::Fail(String::from("Nested box value wrong"));
        }
        SafetyResult::Pass
    }
}

/// Alignment test
pub struct AlignmentTest;

impl SafetyCheck for AlignmentTest {
    fn name(&self) -> &str {
        "alignment"
    }

    fn run(&mut self) -> SafetyResult {
        #[repr(align(16))]
        struct Aligned16 {
            value: u64,
        }

        let a = Box::new(Aligned16 { value: 42 });
        let ptr = &*a as *const Aligned16 as usize;

        if ptr % 16 != 0 {
            return SafetyResult::Fail(alloc::format!("Not aligned: {:x}", ptr));
        }
        SafetyResult::Pass
    }
}

/// Run all safety tests
pub fn run_all_tests() -> Vec<(String, SafetyResult)> {
    let mut results = Vec::new();

    let mut tests: Vec<Box<dyn SafetyCheck>> = vec![
        Box::new(HeapAllocTest),
        Box::new(VecBoundsTest),
        Box::new(MutexSafetyTest),
        Box::new(ArcRefCountTest),
        Box::new(BoxAllocTest),
        Box::new(AlignmentTest),
    ];

    for test in tests.iter_mut() {
        let result = test.run();
        results.push((String::from(test.name()), result));
    }

    results
}
