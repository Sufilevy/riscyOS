use crate::memory::align_up;
use core::{alloc::Layout, mem};

pub(super) struct Node {
    size: usize,
    next: Option<&'static mut Node>,
}

impl Node {
    pub(super) const fn new(size: usize) -> Self {
        Node { size, next: None }
    }

    pub(super) fn start_addr(&self) -> usize {
        self as *const Self as usize
    }

    pub(super) fn end_addr(&self) -> usize {
        self.start_addr() + self.size
    }
}

pub(super) struct LinkedListAllocator {
    head: Node,
}

impl LinkedListAllocator {
    pub(super) const fn new() -> Self {
        Self { head: Node::new(0) }
    }

    /// Initializes the allocator inside the given region.
    pub(super) fn init(&mut self, start: usize, size: usize) {
        self.add_free_region(start, size);
    }

    /// Adds a memory region to the front of the list.
    pub(super) fn add_free_region(&mut self, addr: usize, size: usize) {
        // Check if the address is aligned and the size is big enough
        assert_eq!(align_up(addr, mem::align_of::<Node>()), addr);
        assert!(size >= mem::size_of::<Node>());

        let mut node = Node::new(size);
        node.next = self.head.next.take();
        let node_ptr = addr as *mut Node;
        unsafe {
            node_ptr.write(node);
            self.head.next = Some(&mut *node_ptr);
        }
    }

    /// Searches the linked list for a free region with `size` ans `align` and removes it from the list.
    ///
    /// Returns (node, address) of the region
    pub(super) fn find_region(
        &mut self,
        size: usize,
        align: usize,
    ) -> Option<(&'static mut Node, usize)> {
        let mut current = &mut self.head;

        // Search the list for a large enough region
        while let Some(ref mut region) = current.next {
            if let Some(alloc_start) = Self::alloc_from_region(region, size, align) {
                // The region is good, remove it's node from the list
                let next = region.next.take();
                let ret = Some((current.next.take().unwrap(), alloc_start));
                current.next = next;
                return ret;
            } else {
                // The region is not good, go to the next node
                current = current.next.as_mut().unwrap();
            }
        }

        // No good region found :(
        None
    }

    /// Try to allocate this region with `size` and `align`.
    ///
    /// Returns the start address if successful.
    pub(super) fn alloc_from_region(region: &mut Node, size: usize, align: usize) -> Option<usize> {
        let start = align_up(region.start_addr(), align);
        let end = start.checked_add(size)?;

        if end > region.end_addr() {
            // The region is too small
            return None;
        }

        let remaining_size = region.end_addr() - end;
        if remaining_size > 0 && remaining_size < mem::size_of::<Node>() {
            // The rest of the region is too small to hold a Node
            return None;
        }

        // Region is good :)
        Some(start)
    }

    /// Adjusts `layout` to be a valid region.
    ///
    /// Returns (size, align) of the region.
    pub(super) fn size_align(layout: Layout) -> (usize, usize) {
        let layout = layout
            .align_to(mem::align_of::<Node>())
            .expect("Failed to align layout.")
            .pad_to_align();
        let size = layout.size().max(mem::size_of::<Node>());
        (size, layout.align())
    }
}
