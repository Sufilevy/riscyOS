use super::{
    consts::{FRAME_SIZE, KIB},
    paging::PageEntryLevel,
};
use core::{cmp::Ordering, ptr, slice};
use spin::mutex::SpinMutex;

const BITMAP_ENTRY_BITS: usize = 64;
const BITMAP_ENTRY_SIZE_BYTES: usize = BITMAP_ENTRY_BITS * FRAME_SIZE;

pub static FRAMES_ALLOCATOR: SpinMutex<BitmapAllocator> = SpinMutex::new(BitmapAllocator::new());

pub struct BitmapAllocator {
    bitmap: *mut u64,
    size: usize,
    mem_start: *mut u8,
    mem_end: *mut u8,
}

impl BitmapAllocator {
    pub const fn new() -> Self {
        Self {
            bitmap: ptr::null_mut(),
            size: 0,
            mem_start: ptr::null_mut(),
            mem_end: ptr::null_mut(),
        }
    }

    pub fn bitmap_slice(&mut self) -> &'static mut [u64] {
        unsafe { slice::from_raw_parts_mut(self.bitmap, self.size) }
    }

    /// Returns (entry_index, entry_bit)
    fn bitmap_entry_index_bit(&self, address: usize) -> (usize, usize) {
        (
            (address - self.mem_start as usize) / BITMAP_ENTRY_SIZE_BYTES,
            ((address - self.mem_start as usize) / FRAME_SIZE) % BITMAP_ENTRY_BITS,
        )
    }

    fn set_used(&mut self, address: usize) {
        let (index, bit) = self.bitmap_entry_index_bit(address);
        let entry = &mut self.bitmap_slice()[index];

        *entry |= 1 << bit;
    }

    fn set_unused(&mut self, address: usize) {
        let (index, bit) = self.bitmap_entry_index_bit(address);
        let entry = &mut self.bitmap_slice()[index];

        *entry &= !(1 << bit);
    }

    pub fn init(&mut self, start: *mut u8, end: *mut u8) {
        self.mem_start = start;
        self.mem_end = end;

        // Calculate the size of the bitmap
        let num_frames = (end as usize - start as usize) / FRAME_SIZE;
        self.bitmap = start.cast();
        self.size = num_frames / BITMAP_ENTRY_BITS + 1;

        println!(
            "Bitmap: {{ Start: {:#p}, End: {:#p}, Size: {:#X} }}",
            self.bitmap, self.mem_end, self.size
        );
        self.bitmap_slice().fill(0); // Clear the bitmap

        // Go over the frames required to store the bitmap and mark them as used
        for frame in 0..(self.size / FRAME_SIZE + 1) {
            unsafe {
                self.set_used(self.mem_start.add(frame * FRAME_SIZE) as usize);
            }
        }
    }

    pub fn alloc(&mut self, num_frames: usize, level: PageEntryLevel) -> *mut u8 {
        if num_frames == 1 {
            self.alloc_single(level)
        } else {
            self.alloc_contigous(num_frames, level)
        }
    }

    pub fn zero_alloc(&mut self, num_frames: usize, level: PageEntryLevel) -> *mut u8 {
        let page = self.alloc(num_frames, level); // Allocate a page

        // Cast the pointer to a big int (u64) to force a double-word store instruction and save time.
        // The required stores are 4096 (bytes) / (64 (bits) / 8)
        let ptr = page.cast::<u64>();
        for i in 0..(FRAME_SIZE / 8) {
            unsafe {
                *ptr.add(i) = 0;
            }
        }

        page
    }

    fn alloc_single(&mut self, level: PageEntryLevel) -> *mut u8 {
        match level {
            PageEntryLevel::KiB4 => {
                // Find an entry in the bitmap that is not completly filled
                if let Some((index, entry)) = self
                    .bitmap_slice()
                    .iter_mut()
                    .enumerate()
                    .find(|(_, e)| **e != u64::MAX)
                {
                    let bit_index = entry.trailing_ones() as usize; // Calculate the index of the free bit

                    // Calculate the frame's address (entry's first frame's address + free entry's index * FRAME_SIZE)
                    let frame_ptr = ((self.mem_start as usize + index * BITMAP_ENTRY_SIZE_BYTES)
                        + (bit_index * FRAME_SIZE)) as *mut u8;

                    if frame_ptr <= self.mem_end {
                        *entry |= 1 << bit_index;
                        return frame_ptr;
                    }
                }
                panic!("Could not find enough frames to allocate.");
            }
            PageEntryLevel::MiB2 => self.alloc_contigous(1, PageEntryLevel::MiB2),
            PageEntryLevel::GiB1 => todo!("Frame allocation by GB."),
        }
    }

    fn alloc_contigous(&mut self, num_frames: usize, level: PageEntryLevel) -> *mut u8 {
        // Allocate 4KiB frames
        if let PageEntryLevel::KiB4 = level {
            if num_frames <= BITMAP_ENTRY_BITS {
                return self.intra_alloc_contigous_4k_frames(num_frames); // Allocate 64 or less frames
            } else {
                return self.inter_alloc_contigous_4k_frames(num_frames); // Allocate more than 64 frames
            }
        }

        // Allocate 2MiB or 2GiB frames
        // Calculate the number of bitmap entries needed for the alloc (maximum 1)
        let num_entries = (((level.size() / FRAME_SIZE) * num_frames) / BITMAP_ENTRY_BITS).max(1);
        let mut start_index = self.mem_start.align_offset(level.size()) / (BITMAP_ENTRY_BITS * KIB);
        let mut end_index = start_index + num_entries;

        // Find a big enough group of contigous empty entries
        while self
            .bitmap_slice()
            .get(start_index..end_index)
            .unwrap()
            .iter()
            .any(|entry| entry.count_ones() != 0)
        {
            start_index += num_entries;
            end_index = start_index + num_entries;
        }

        // Mark the entire group of frames as taken
        for entry in &mut self.bitmap_slice()[start_index..end_index] {
            *entry = u64::MAX;
        }

        let page_ptr = self.mem_start as usize + start_index * BITMAP_ENTRY_SIZE_BYTES;
        assert!(page_ptr % level.size() == 0, "Allocation is not aligned.");
        page_ptr as *mut u8
    }

    /// Allocate 64 or less contigous frames
    fn intra_alloc_contigous_4k_frames(&mut self, num_frames: usize) -> *mut u8 {
        // Check if we need to allocate a whole entry
        if num_frames == BITMAP_ENTRY_BITS {
            // Find an empty entry
            let (index, entry) = self
                .bitmap_slice()
                .iter_mut()
                .enumerate()
                .find(|(_, e)| **e == 0)
                .unwrap();
            *entry = u64::MAX; // Mark the entry as filled

            let page_ptr = (self.mem_start as usize + index * BITMAP_ENTRY_SIZE_BYTES) as *mut u8;

            if page_ptr < self.mem_end {
                return page_ptr;
            } else {
                panic!("Could not find enough contigous frames to allocate.");
            }
        }

        let mask = u64::MAX << num_frames; // The bitmask of the required number of frames

        // Find an entry with enough free frames
        let free_bits_filter = |(_, e): &(usize, &mut u64)| e.count_zeros() as usize >= num_frames;
        for (index, entry) in self
            .bitmap_slice()
            .iter_mut()
            .enumerate()
            .filter(free_bits_filter)
        {
            let bit_index = match (0..(BITMAP_ENTRY_BITS - num_frames))
                .map(|i| (i, *entry >> i))
                .find(|(_, e)| e | mask == mask)
            {
                Some((i, _)) => i, // We have found a match
                None => continue,  // Search in another entry
            };

            *entry |= (!mask).rotate_left(bit_index as u32); // Mark the allocated bits as used

            let page_ptr = ((self.mem_start as usize + index * BITMAP_ENTRY_SIZE_BYTES)
                + bit_index * FRAME_SIZE) as *mut u8; // Calculate the pointer to the found page

            if page_ptr < self.mem_end {
                return page_ptr;
            } else {
                panic!("Could not find enough contigous frames to allocate.");
            }
        }

        panic!("Could not find enough contigous frames to allocate.");
    }

    /// Allocate more than 64 contigous frames
    fn inter_alloc_contigous_4k_frames(&mut self, num_frames: usize) -> *mut u8 {
        let entries_needed = num_frames / BITMAP_ENTRY_BITS;
        let remaining_bits_needed = (num_frames % BITMAP_ENTRY_BITS) as u32;

        let mut start_index = 0;
        let bitmap = self.bitmap_slice();
        while self.mem_start as usize + start_index < self.mem_end as usize - entries_needed - 1 {
            let range = start_index..(start_index + entries_needed);

            // Check if any of the next entries are not empty
            if bitmap.get(range.clone()).unwrap().iter().any(|e| *e != 0) {
                start_index += entries_needed; // If yes, skip to the next batch
                continue;
            }

            // Check if there is enough space for the remaining bits in the entry right after the batch
            if remaining_bits_needed != 0
                && bitmap.get(range.end).unwrap().leading_zeros() < remaining_bits_needed
            {
                start_index += range.end + 1;
                continue;
            }

            let page_ptr =
                (self.mem_start as usize + start_index * BITMAP_ENTRY_SIZE_BYTES) as *mut u8;

            if page_ptr < self.mem_end {
                // Mark the allocated entries as used
                bitmap
                    .get_mut(range.clone())
                    .unwrap()
                    .iter_mut()
                    .for_each(|e| *e = u64::MAX);
                if remaining_bits_needed > 0 {
                    *bitmap.get_mut(range.end).unwrap() |= !(u64::MAX << remaining_bits_needed);
                }

                return page_ptr;
            } else {
                panic!("Could not find enough contigous frames to allocate.");
            }
        }

        panic!("Could not find enough contigous frames to allocate.");
    }

    pub fn dealloc(&mut self, address: usize, size: usize, level: PageEntryLevel) {
        if size == 1 {
            self.dealloc_single(address, level);
        } else {
            self.dealloc_contigous(address, size, level);
        }
    }

    fn dealloc_single(&mut self, address: usize, level: PageEntryLevel) {
        match level {
            PageEntryLevel::KiB4 => {
                let (index, bit) = self.bitmap_entry_index_bit(address);
                let entry = &mut self.bitmap_slice()[index];

                // Check if we're trying to free an already freed frame
                if (*entry >> bit) & 1 != 1 {
                    panic!("Double free detected at: {:#p}", address as *const u8);
                }

                *entry &= !(1 << bit); // Mark the frame as free
            }
            PageEntryLevel::MiB2 => {
                let (index, _) = self.bitmap_entry_index_bit(address);
                for entry in &mut self.bitmap_slice()[index..][..512] {
                    // Check if we're trying to free an already freed entry
                    assert!(
                        *entry == u64::MAX,
                        "Doulbe free of 64 frames found at: {:#p}",
                        address as *const u8,
                    );

                    *entry = 0; // Mark the current entry as free
                }
            }
            PageEntryLevel::GiB1 => todo!("Frame allocation by GB."),
        }
    }

    fn dealloc_contigous(&mut self, address: usize, size: usize, level: PageEntryLevel) {
        let (index, bit) = self.bitmap_entry_index_bit(address);

        match level {
            PageEntryLevel::KiB4 => {
                match size.cmp(&BITMAP_ENTRY_BITS) {
                    Ordering::Less => {
                        // Deallocate less than 64 frames
                        let mask = (u64::MAX << size).rotate_left(bit as u32);
                        self.bitmap_slice()[index] &= mask;
                    }
                    Ordering::Equal => {
                        // Deallocate 64 frames
                        self.bitmap_slice()[index] = 0;
                    }
                    Ordering::Greater => {
                        // Deallocate more than 64 frames
                        let entries_needed = size / BITMAP_ENTRY_BITS;
                        let remaining_bits_needed = (size % BITMAP_ENTRY_BITS) as u32;
                        let range = index..(index + entries_needed);

                        for entry in &mut self.bitmap_slice()[range.clone()] {
                            // Check if we're trying to free an already freed entry
                            assert!(
                                *entry == u64::MAX,
                                "Doulbe free of 64 frames found at: {:#p}",
                                address as *const u8,
                            );
                            *entry = 0; // Mark the entry as free
                        }

                        if remaining_bits_needed > 0 {
                            self.bitmap_slice()[range.end] &= u64::MAX << remaining_bits_needed;
                        }
                    }
                }
            }
            _ => {
                // Calculate the number of bitmap entries needed for the dealloc (maximum 1)
                let num_entries = (((level.size() / FRAME_SIZE) * size) / 64).max(1);
                let end_index = index + num_entries;

                for entry in &mut self.bitmap_slice()[index..][..end_index] {
                    // Check if we're trying to free an already freed entry
                    assert!(
                        *entry == u64::MAX,
                        "Doulbe free of 64 frames found at: {:#p}",
                        address as *const u8,
                    );
                    *entry = 0; // Mark the entry as free
                }
            }
        }
    }
}

unsafe impl Send for BitmapAllocator {}
unsafe impl Sync for BitmapAllocator {}

pub fn init_frames_allocation(start: *mut u8, size: usize) {
    let start = unsafe { start.add(start.align_offset(u64::BITS as usize)) };
    FRAMES_ALLOCATOR
        .lock()
        .init(start, (start as usize + size) as *mut u8);
}
