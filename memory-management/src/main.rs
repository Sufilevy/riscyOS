#![feature(const_mut_refs)]

mod memory;

fn main() {
    let mem_size = 0x800000;
    let mut mem = memory::virt::init_virtual_memory(mem_size);
    let mem_start = mem.as_mut_ptr();
    println!("* Initiated virtual memory at {mem_start:?}.");

    memory::init_frames_allocation(mem_start, mem_size);
    println!("* Initiated frames allocation.");

    let root_table = memory::paging::create_root_table();
    println!("* Created root table at: {root_table:?}");
    memory::map_kernel(unsafe { root_table.as_mut() }.unwrap());
    println!("* Mapped kernel.");

    memory::alloc::init(mem_start, mem_size - 4096);
    println!("* Initiated paging.");
}
