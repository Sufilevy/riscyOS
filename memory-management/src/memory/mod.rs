pub mod alloc;
pub mod consts;
mod frames;
pub mod paging;
pub mod virt;

use consts::*;
pub use frames::init_frames_allocation;
use paging::{PageEntryFlags, PageEntryLevel};

/// Aligns `value` to 2 to the power of `order`. Always rounds up.
pub const fn align_order(value: usize, order: usize) -> usize {
    let o = (1usize << order) - 1;
    (value + o) & !o
}

/// Aligns `value` to `align`. Always rounds up.
pub const fn align_up(value: usize, align: usize) -> usize {
    let remainder = value % align;
    if remainder == 0 {
        value // Already aligned
    } else {
        value + align - remainder
    }
}

/// Identity map means that the virtual address is equal to the physical address.
pub fn identity_map_range(
    root: &mut paging::PageTable,
    start: usize,
    end: usize,
    flags: PageEntryFlags,
    level: PageEntryLevel,
) {
    let page_size = level.size();

    let start = align_order(start, page_size.ilog2() as usize);
    let end = align_order(end, page_size.ilog2() as usize);

    for addr in (start..end).step_by(page_size) {
        paging::map(root, addr, addr, &flags, level);
    }
}

macro_rules! map_region {
    ($root:ident, $start:ident, $end:ident, $flags:expr) => {
        identity_map_range(
            $root,
            $start,
            $end,
            $flags | PageEntryFlags::ACCESSED_DIRTY,
            PageEntryLevel::from_size($end - $start),
        );
    };
}

pub fn map_kernel(root: &mut paging::PageTable) {
    // Map text (code)
    map_region!(root, TEXT_START, TEXT_END, PageEntryFlags::READ_EXECUTE);
    println!("Mapped text.");

    // Map read-only-data (constants)
    map_region!(root, RODATA_START, RODATA_END, PageEntryFlags::READ);
    println!("Mapped rodata.");

    // Map data (initialized variables)
    map_region!(root, DATA_START, DATA_END, PageEntryFlags::READ_WRITE);
    println!("Mapped data.");

    // Map block-starting-symbol (zero-initialized variables)
    map_region!(root, BSS_START, BSS_END, PageEntryFlags::READ_WRITE);
    println!("Mapped bss.");

    // Map stack
    map_region!(root, STACK_START, STACK_END, PageEntryFlags::READ_WRITE);
    println!("Mapped stack.");

    // Map heap
    map_region!(root, HEAP_START, HEAP_END, PageEntryFlags::READ_WRITE);
    println!("Mapped heap.");
}
