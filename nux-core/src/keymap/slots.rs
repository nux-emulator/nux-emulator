//! Touch slot allocator for multi-touch protocol.
//!
//! Android's multi-touch type-B protocol uses numbered slots. This allocator
//! manages a fixed pool of slots so multiple simultaneous bindings (e.g.
//! joystick + tap) each get a unique slot.

/// Maximum number of simultaneous touch slots.
const MAX_SLOTS: u32 = 10;

/// Allocates and releases multi-touch slot IDs.
#[derive(Debug)]
pub struct SlotAllocator {
    /// Bitfield tracking which slots are in use.
    used: u32,
}

impl Default for SlotAllocator {
    fn default() -> Self {
        Self::new()
    }
}

impl SlotAllocator {
    /// Create a new allocator with all slots free.
    #[must_use]
    pub fn new() -> Self {
        Self { used: 0 }
    }

    /// Allocate the next free slot.
    ///
    /// Returns `None` if all slots are exhausted.
    pub fn allocate(&mut self) -> Option<u32> {
        for i in 0..MAX_SLOTS {
            if self.used & (1 << i) == 0 {
                self.used |= 1 << i;
                return Some(i);
            }
        }
        None
    }

    /// Release a previously allocated slot.
    pub fn release(&mut self, slot: u32) {
        if slot < MAX_SLOTS {
            self.used &= !(1 << slot);
        }
    }

    /// Reset all slots to free.
    pub fn reset(&mut self) {
        self.used = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocate_returns_sequential_slots() {
        let mut alloc = SlotAllocator::new();
        assert_eq!(alloc.allocate(), Some(0));
        assert_eq!(alloc.allocate(), Some(1));
        assert_eq!(alloc.allocate(), Some(2));
    }

    #[test]
    fn exhaustion_returns_none() {
        let mut alloc = SlotAllocator::new();
        for _ in 0..MAX_SLOTS {
            assert!(alloc.allocate().is_some());
        }
        assert_eq!(alloc.allocate(), None);
    }

    #[test]
    fn release_allows_reuse() {
        let mut alloc = SlotAllocator::new();
        let s0 = alloc.allocate().unwrap();
        let _s1 = alloc.allocate().unwrap();
        alloc.release(s0);
        // Slot 0 should be available again
        assert_eq!(alloc.allocate(), Some(0));
    }

    #[test]
    fn reset_frees_all() {
        let mut alloc = SlotAllocator::new();
        for _ in 0..5 {
            alloc.allocate();
        }
        alloc.reset();
        assert_eq!(alloc.allocate(), Some(0));
    }
}
