use std::{
    fs::{File, OpenOptions},
    io::{Result, Seek, Write},
};

use memmap::MmapMut;

fn create_memory_file(path: &str, size: usize) -> Result<()> {
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(path)?;

    // Set the size of the file to the desired memory size
    file.set_len(size as u64)?;

    // Initialize the memory to all zeros
    file.rewind()?;
    let zeros = vec![0u8; size];
    file.write_all(&zeros)?;

    Ok(())
}

pub fn init_virtual_memory(mem_size: usize) -> MmapMut {
    // Create the memory file with the desired size
    create_memory_file("mem.img", mem_size).expect("Failed to create memory file");
    println!("Created memory file.");

    // Open the memory file and map it into memory
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open("mem.img")
        .expect("Failed to open memory file");

    unsafe {
        memmap::MmapOptions::new()
            .len(mem_size)
            .map_mut(&file)
            .expect("Failed to map memory file into memory")
    }
}
