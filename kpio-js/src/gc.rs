//! Garbage collector for JavaScript values.
//!
//! Simple mark-and-sweep garbage collector.

use alloc::vec;
use alloc::vec::Vec;
use alloc::rc::Rc;
use core::cell::RefCell;

use crate::object::JsObject;
use crate::value::Value;

/// Garbage collector.
pub struct GarbageCollector {
    /// All tracked objects.
    objects: Vec<Rc<RefCell<JsObject>>>,
    /// Collection threshold.
    threshold: usize,
    /// Objects allocated since last collection.
    allocations: usize,
}

impl GarbageCollector {
    /// Create a new garbage collector.
    pub fn new() -> Self {
        GarbageCollector {
            objects: Vec::new(),
            threshold: 1000,
            allocations: 0,
        }
    }
    
    /// Create with custom threshold.
    pub fn with_threshold(threshold: usize) -> Self {
        GarbageCollector {
            objects: Vec::new(),
            threshold,
            allocations: 0,
        }
    }
    
    /// Track an object.
    pub fn track(&mut self, obj: Rc<RefCell<JsObject>>) {
        self.objects.push(obj);
        self.allocations += 1;
    }
    
    /// Check if collection is needed.
    pub fn should_collect(&self) -> bool {
        self.allocations >= self.threshold
    }
    
    /// Collect garbage given root values.
    pub fn collect(&mut self, roots: &[Value]) {
        // Mark phase
        let marked = self.mark(roots);
        
        // Sweep phase
        self.sweep(&marked);
        
        // Reset allocation counter
        self.allocations = 0;
    }
    
    /// Mark reachable objects.
    fn mark(&self, roots: &[Value]) -> Vec<bool> {
        let mut marked = vec![false; self.objects.len()];
        
        for root in roots {
            self.mark_value(root, &mut marked);
        }
        
        marked
    }
    
    /// Mark a value and its references.
    fn mark_value(&self, value: &Value, marked: &mut [bool]) {
        if let Value::Object(obj) = value {
            // Find this object in our list
            for (i, tracked) in self.objects.iter().enumerate() {
                if Rc::ptr_eq(obj, tracked) {
                    if marked[i] {
                        return; // Already marked
                    }
                    marked[i] = true;
                    
                    // Mark referenced objects
                    // In a real implementation, we would iterate through
                    // all properties and mark their values
                    break;
                }
            }
        }
    }
    
    /// Sweep unmarked objects.
    fn sweep(&mut self, marked: &[bool]) {
        let mut i = 0;
        while i < self.objects.len() {
            if !marked[i] {
                // Object is not reachable, remove it
                self.objects.swap_remove(i);
            } else {
                i += 1;
            }
        }
    }
    
    /// Get statistics.
    pub fn stats(&self) -> GcStats {
        GcStats {
            total_objects: self.objects.len(),
            allocations_since_gc: self.allocations,
            threshold: self.threshold,
        }
    }
    
    /// Clear all tracked objects.
    pub fn clear(&mut self) {
        self.objects.clear();
        self.allocations = 0;
    }
}

impl Default for GarbageCollector {
    fn default() -> Self {
        Self::new()
    }
}

/// GC statistics.
#[derive(Clone, Debug)]
pub struct GcStats {
    /// Total tracked objects.
    pub total_objects: usize,
    /// Allocations since last GC.
    pub allocations_since_gc: usize,
    /// Collection threshold.
    pub threshold: usize,
}

/// GC handle for values.
#[derive(Clone)]
pub struct GcHandle<T> {
    value: Rc<RefCell<T>>,
}

impl<T> GcHandle<T> {
    /// Create a new GC handle.
    pub fn new(value: T) -> Self {
        GcHandle {
            value: Rc::new(RefCell::new(value)),
        }
    }
    
    /// Get the inner Rc.
    pub fn inner(&self) -> Rc<RefCell<T>> {
        self.value.clone()
    }
    
    /// Check if this is the only reference.
    pub fn is_unique(&self) -> bool {
        Rc::strong_count(&self.value) == 1
    }
    
    /// Get reference count.
    pub fn ref_count(&self) -> usize {
        Rc::strong_count(&self.value)
    }
}

/// Arena allocator for objects.
pub struct ObjectArena {
    /// Object storage.
    objects: Vec<Option<JsObject>>,
    /// Free list.
    free_list: Vec<usize>,
}

impl ObjectArena {
    /// Create a new arena.
    pub fn new() -> Self {
        ObjectArena {
            objects: Vec::new(),
            free_list: Vec::new(),
        }
    }
    
    /// Create with initial capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        ObjectArena {
            objects: Vec::with_capacity(capacity),
            free_list: Vec::new(),
        }
    }
    
    /// Allocate an object.
    pub fn alloc(&mut self, obj: JsObject) -> usize {
        if let Some(index) = self.free_list.pop() {
            self.objects[index] = Some(obj);
            index
        } else {
            let index = self.objects.len();
            self.objects.push(Some(obj));
            index
        }
    }
    
    /// Free an object.
    pub fn free(&mut self, index: usize) {
        if index < self.objects.len() && self.objects[index].is_some() {
            self.objects[index] = None;
            self.free_list.push(index);
        }
    }
    
    /// Get an object.
    pub fn get(&self, index: usize) -> Option<&JsObject> {
        self.objects.get(index).and_then(|o| o.as_ref())
    }
    
    /// Get a mutable object.
    pub fn get_mut(&mut self, index: usize) -> Option<&mut JsObject> {
        self.objects.get_mut(index).and_then(|o| o.as_mut())
    }
    
    /// Get statistics.
    pub fn stats(&self) -> ArenaStats {
        ArenaStats {
            total_slots: self.objects.len(),
            used_slots: self.objects.len() - self.free_list.len(),
            free_slots: self.free_list.len(),
        }
    }
    
    /// Clear the arena.
    pub fn clear(&mut self) {
        self.objects.clear();
        self.free_list.clear();
    }
}

impl Default for ObjectArena {
    fn default() -> Self {
        Self::new()
    }
}

/// Arena statistics.
#[derive(Clone, Debug)]
pub struct ArenaStats {
    /// Total slots.
    pub total_slots: usize,
    /// Used slots.
    pub used_slots: usize,
    /// Free slots.
    pub free_slots: usize,
}

/// Simple reference counting GC.
/// 
/// This is a basic implementation that uses Rust's Rc for
/// automatic reference counting. Cyclic references are not
/// automatically collected.
pub struct RefCountGc {
    /// Weak reference count.
    cycles_detected: usize,
}

impl RefCountGc {
    /// Create a new ref-counting GC.
    pub fn new() -> Self {
        RefCountGc {
            cycles_detected: 0,
        }
    }
    
    /// Detect potential cycles in an object graph.
    pub fn detect_cycles(&mut self, roots: &[Value]) -> usize {
        // In a real implementation, this would use Tarjan's algorithm
        // or similar to detect strongly connected components.
        let _ = roots;
        self.cycles_detected
    }
    
    /// Get cycle count.
    pub fn cycle_count(&self) -> usize {
        self.cycles_detected
    }
}

impl Default for RefCountGc {
    fn default() -> Self {
        Self::new()
    }
}
