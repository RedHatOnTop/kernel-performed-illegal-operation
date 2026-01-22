//! Round-robin scheduler policy.
//!
//! This module implements a basic round-robin scheduling policy
//! for tasks within the same priority level.

use alloc::collections::VecDeque;
use alloc::sync::Arc;
use spin::Mutex;

use super::task::{Task, TaskId};

/// Round-robin scheduler for a single priority level.
pub struct RoundRobinQueue {
    /// Queue of ready tasks.
    tasks: VecDeque<Arc<Mutex<Task>>>,
    /// Time quantum in timer ticks.
    quantum: u64,
}

impl RoundRobinQueue {
    /// Create a new round-robin queue.
    pub fn new(quantum: u64) -> Self {
        RoundRobinQueue {
            tasks: VecDeque::new(),
            quantum,
        }
    }
    
    /// Add a task to the end of the queue.
    pub fn enqueue(&mut self, task: Arc<Mutex<Task>>) {
        self.tasks.push_back(task);
    }
    
    /// Remove and return the task at the front of the queue.
    pub fn dequeue(&mut self) -> Option<Arc<Mutex<Task>>> {
        self.tasks.pop_front()
    }
    
    /// Peek at the task at the front without removing it.
    pub fn peek(&self) -> Option<&Arc<Mutex<Task>>> {
        self.tasks.front()
    }
    
    /// Check if the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.tasks.is_empty()
    }
    
    /// Get the number of tasks in the queue.
    pub fn len(&self) -> usize {
        self.tasks.len()
    }
    
    /// Get the time quantum.
    pub fn quantum(&self) -> u64 {
        self.quantum
    }
    
    /// Set the time quantum.
    pub fn set_quantum(&mut self, quantum: u64) {
        self.quantum = quantum;
    }
    
    /// Remove a specific task by ID.
    pub fn remove(&mut self, task_id: TaskId) -> Option<Arc<Mutex<Task>>> {
        let pos = self.tasks.iter().position(|t| t.lock().id() == task_id)?;
        self.tasks.remove(pos)
    }
    
    /// Move a task to the end of the queue (for time slice expiry).
    pub fn rotate(&mut self) {
        if let Some(task) = self.tasks.pop_front() {
            self.tasks.push_back(task);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    // Tests would go here
}
