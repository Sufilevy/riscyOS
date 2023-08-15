mod list;

use core::{alloc::GlobalAlloc, ptr};

use self::list::LinkedListAllocator;

pub struct KernelAllocator<A> {
    allocator: spin::Mutex<A>,
}

impl<A> KernelAllocator<A> {
    pub const fn new(allocator: A) -> Self {
        Self {
            allocator: spin::Mutex::new(allocator),
        }
    }
}

unsafe impl GlobalAlloc for KernelAllocator<LinkedListAllocator> {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        let (size, align) = LinkedListAllocator::size_align(layout);
        let mut allocator = self.allocator.lock();

        // Allocate a new region
        if let Some((region, start)) = allocator.find_region(size, align) {
            let end = start.checked_add(size).expect("Address addition overflow.");
            let remaining_size = region.end_addr() - end;
            if remaining_size > 0 {
                // If there is a remaining region, add it to the list
                allocator.add_free_region(end, remaining_size);
            }
            start as *mut u8
        } else {
            ptr::null_mut() // There are no more regions to allocate
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
        let (size, _) = LinkedListAllocator::size_align(layout);

        self.allocator.lock().add_free_region(ptr as usize, size);
    }
}

static ALLOCATOR: KernelAllocator<LinkedListAllocator> =
    KernelAllocator::new(LinkedListAllocator::new());

pub fn init(mem_start: *mut u8, size: usize) {
    unsafe {
        ALLOCATOR
            .allocator
            .lock()
            .init(mem_start.offset(1024) as usize, size);
    }
}
