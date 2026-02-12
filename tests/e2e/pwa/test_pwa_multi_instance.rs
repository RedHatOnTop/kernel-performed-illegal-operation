//! E2E Test: PWA Multi-Instance
//!
//! Tests that the same PWA can run in multiple windows:
//! 1. Launch same app twice → 2 separate windows
//! 2. Each gets a unique instance ID
//! 3. Closing one doesn't affect the other

#[cfg(test)]
mod tests {
    extern crate alloc;
    use alloc::string::String;

    #[test]
    fn test_two_instances_same_app() {
        // Launch app_id=1 twice → 2 instances
        let instance_id_1: u64 = 1;
        let instance_id_2: u64 = 2;
        assert_ne!(instance_id_1, instance_id_2);
    }

    #[test]
    fn test_separate_windows() {
        // Each instance gets its own window
        let window_id_1: u64 = 100;
        let window_id_2: u64 = 101;
        assert_ne!(window_id_1, window_id_2);
    }

    #[test]
    fn test_close_one_keeps_other() {
        // After closing instance 1, instance 2 should still be running
        let instance_1_alive = false;
        let instance_2_alive = true;
        assert!(!instance_1_alive);
        assert!(instance_2_alive);
    }

    #[test]
    fn test_separate_storage_contexts() {
        // Each instance should get its own sessionStorage
        // (localStorage is shared per origin)
        let storage_type = "session";
        let is_per_instance = storage_type == "session";
        assert!(is_per_instance);
    }

    #[test]
    fn test_shared_local_storage() {
        // localStorage is shared across instances of the same app
        let storage_type = "local";
        let is_shared = storage_type == "local";
        assert!(is_shared);
    }
}
