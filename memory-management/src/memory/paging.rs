#![allow(dead_code)] // REMOVE THIS LINE

use core::ops;
use modular_bitfield::prelude::*;

use crate::memory::frames::FRAMES_ALLOCATOR;

use super::consts::NUM_VPNS;

pub struct PageEntryFlags(u8);

impl PageEntryFlags {
    pub const VALID: Self = Self(1 << 0);
    pub const READ: Self = Self(1 << 1);
    pub const WRITE: Self = Self(1 << 2);
    pub const EXECUTE: Self = Self(1 << 3);
    pub const USER: Self = Self(1 << 4);
    pub const GLOBAL: Self = Self(1 << 5);
    pub const ACCESSED: Self = Self(1 << 6);
    pub const DIRTY: Self = Self(1 << 7);

    // Convenience combinations
    pub const READ_WRITE: Self = Self((1 << 1) | (1 << 2));
    pub const READ_EXECUTE: Self = Self((1 << 1) | (1 << 3));
    pub const READ_WRITE_EXECUTE: Self = Self((1 << 1) | (1 << 2) | (1 << 3));
    pub const ACCESSED_DIRTY: Self = Self((1 << 6) | (1 << 7));

    // User convenience combinations
    pub const USER_READ_WRITE: Self = Self((1 << 1) | (1 << 2) | (1 << 4));
    pub const USER_READ_EXECUTE: Self = Self((1 << 1) | (1 << 3) | (1 << 4));
    pub const USER_READ_WRITE_EXECUTE: Self = Self((1 << 1) | (1 << 2) | (1 << 3) | (1 << 4));

    pub fn val(self) -> u8 {
        self.0
    }

    pub fn is_leaf(&self) -> bool {
        self.contains(Self::READ) || self.contains(Self::WRITE) || self.contains(Self::EXECUTE)
    }

    pub fn contains(&self, other: Self) -> bool {
        self.0 & other.0 != 0
    }
}

impl ops::BitOr for PageEntryFlags {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl ops::BitAnd for PageEntryFlags {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        Self(self.0 & rhs.0)
    }
}

pub enum PageEntryType {
    Invalid,
    Leaf,
    Branch(usize),
}

#[bitfield(bits = 64)]
#[derive(Debug)]
pub struct PageEntry {
    valid: bool,
    read: bool,
    write: bool,
    execute: bool,
    user: bool,
    global: bool,
    accessed: bool,
    dirty: bool,
    #[skip(getters, setters)]
    reserved_for_software: B2,
    ppn0: B9,
    ppn1: B9,
    ppn2: B26,
    #[skip(getters, setters)]
    reserved: B10,
}

impl PageEntry {
    pub fn is_leaf(&self) -> bool {
        self.read() || self.write() || self.execute()
    }

    pub fn is_branch(&self) -> bool {
        !self.is_leaf()
    }

    pub fn is_valid(&self) -> bool {
        self.valid()
    }

    pub fn is_invalid(&self) -> bool {
        !self.valid()
    }

    pub fn get_type(&self) -> PageEntryType {
        if self.is_invalid() {
            return PageEntryType::Invalid;
        }

        if self.is_leaf() {
            return PageEntryType::Leaf;
        }

        PageEntryType::Branch(self.get_ppn())
    }

    pub fn set_ppn(&mut self, ppn: usize) {
        self.set_ppn0(((ppn >> 12) & 0x1ff) as u16); // PPN[0] = physical_addr[12:20]
        self.set_ppn1(((ppn >> 21) & 0x1ff) as u16); // PPN[1] = physical_addr[21:29]
        self.set_ppn2(((ppn >> 30) & 0x3ff_ffff) as u32); // PPN[2] = physical_addr[30:55]
    }

    pub fn get_ppn(&self) -> usize {
        ((self.ppn0() as usize) << 12)
            | ((self.ppn1() as usize) << 21)
            | ((self.ppn2() as usize) << 30)
    }

    pub fn set_flags(&mut self, flags: &PageEntryFlags) {
        self.set_valid(flags.contains(PageEntryFlags::VALID));
        self.set_read(flags.contains(PageEntryFlags::READ));
        self.set_write(flags.contains(PageEntryFlags::WRITE));
        self.set_execute(flags.contains(PageEntryFlags::EXECUTE));
        self.set_user(flags.contains(PageEntryFlags::USER));
        self.set_global(flags.contains(PageEntryFlags::GLOBAL));
        self.set_accessed(flags.contains(PageEntryFlags::ACCESSED));
        self.set_dirty(flags.contains(PageEntryFlags::DIRTY));
    }

    pub fn copy_flags(&mut self, entry: PageEntry) {
        self.set_valid(entry.valid());
        self.set_read(entry.read());
        self.set_write(entry.write());
        self.set_execute(entry.execute());
        self.set_user(entry.user());
        self.set_global(entry.global());
        self.set_accessed(entry.accessed());
        self.set_dirty(entry.dirty());
    }

    pub fn extract_vpns(vpn: usize) -> [usize; NUM_VPNS] {
        // Extract the parts of the VPN. Each part is 9 bits (0x1FF = 0b1_1111_1111).
        // We ignore the first 12 bits because they are the frame offset (there are 2^12 = 4096 addresses in a frame).
        [
            (vpn >> 12) & 0x1FF, // VPN[0] = virtual_addr[12:20]
            (vpn >> 21) & 0x1FF, // VPN[1] = virtual_addr[21:29]
            (vpn >> 30) & 0x1FF, // VPN[2] = virtual_addr[30:38]
        ]
    }
}

#[repr(usize)]
#[derive(Clone, Copy, PartialEq)]
pub enum PageEntryLevel {
    KiB4 = 0,
    MiB2 = 1,
    GiB1 = 2,
}

impl PageEntryLevel {
    pub fn top() -> Self {
        Self::GiB1
    }

    pub fn val(self) -> usize {
        self as usize
    }

    pub fn size(self) -> usize {
        4096 * 512usize.pow(self as u32)
    }

    pub fn from_size(size: usize) -> Self {
        if size > Self::GiB1.size() {
            return Self::GiB1;
        } else if size > Self::MiB2.size() {
            return Self::MiB2;
        }
        PageEntryLevel::KiB4
    }

    pub fn next_level(self) -> Option<Self> {
        match self {
            Self::KiB4 => None,
            Self::MiB2 => Some(Self::KiB4),
            Self::GiB1 => Some(Self::MiB2),
        }
    }

    pub fn assert_aligned(self, addr: usize) {
        assert!(
            addr % self.size() == 0,
            "Address is not aligned with page size"
        )
    }
}

const PAGE_TABLE_LEN: usize = 512;

#[repr(align(4096))]
pub struct PageTable {
    pub entries: [PageEntry; PAGE_TABLE_LEN],
}

/// root - A mutable reference to the root of the page table (level 2).
/// from_addr - The physical address.
/// to_addr - The virtual address.
/// entry_flags - Any additional flags of the entry (Read, Write, Execute, etc.)
/// level - The level in which the page will be mapped
pub fn map(
    root: &mut PageTable,
    from_addr: usize,
    to_addr: usize,
    entry_flags: &PageEntryFlags,
    level: PageEntryLevel,
) {
    println!(
        "- Mapping: {:#X} -> {:#X} | FLAGS={:#b} | LEVEL={}",
        from_addr,
        to_addr,
        entry_flags.0,
        level.val()
    );
    level.assert_aligned(to_addr);
    level.assert_aligned(from_addr);
    assert!(entry_flags.is_leaf(), "Cannot map branch");

    // Extract the parts of the VPN.
    let vpns = PageEntry::extract_vpns(to_addr);

    let mut table = root;
    let mut current_level = PageEntryLevel::top(); // Start from the top level

    // Traverse the page table (the root is expected to be valid, but the rest can be created)
    for vpn in vpns.into_iter().rev() {
        // A reference to the current entry that we're on (can be level 2, 1 or 0)
        let entry = &mut table.entries[vpn];

        if current_level == level {
            if entry.is_valid() {
                panic!(
                    "Attempt to map an already mapped address: {:#X} -> {:#X}",
                    from_addr, to_addr
                );
            }

            entry.set_flags(entry_flags);
            entry.set_ppn(from_addr);

            return;
        }

        match entry.get_type() {
            PageEntryType::Leaf => panic!("Tried to go through leaf instead of branch"),
            PageEntryType::Branch(next_addr) => {
                table = unsafe { (next_addr as *mut PageTable).as_mut().unwrap() }
            }
            PageEntryType::Invalid => {
                let subtable = FRAMES_ALLOCATOR
                    .lock()
                    .zero_alloc(1, PageEntryLevel::KiB4)
                    .cast::<PageTable>();
                entry.set_valid(true);
                entry.set_ppn(subtable as usize);

                table = unsafe { subtable.as_mut().unwrap() };
            }
        }

        current_level = match current_level.next_level() {
            Some(level) => level,
            None => panic!("There is no page size smaller than 4KiB"),
        }
    }
}

/// Unmap and free all of the memory of this table (doesn't have to be root)
pub fn unmap(table: &mut PageTable) {
    for level2 in 0..PAGE_TABLE_LEN {
        let entry_level2 = &table.entries[level2];
        if entry_level2.is_valid() && entry_level2.is_branch() {
            // This is a branch, free all of the other entries
            let ptr_level1 = entry_level2.get_ppn();
            let table_level1 = unsafe { (ptr_level1 as *mut PageTable).as_mut().unwrap() };
            for level1 in 0..PAGE_TABLE_LEN {
                let entry_level1 = &table_level1.entries[level1];
                if entry_level1.is_valid() && entry_level1.is_branch() {
                    // This is a branch, free all of the other entries
                    let ptr_level0 = entry_level1.get_ppn();
                    FRAMES_ALLOCATOR
                        .lock()
                        .dealloc(ptr_level0, 1, PageEntryLevel::KiB4);
                }
            }
            FRAMES_ALLOCATOR
                .lock()
                .dealloc(ptr_level1, 1, PageEntryLevel::MiB2);
        }
    }
}

/// Convert a virtual address to a physical address by walking the page table.
/// If a page fault occurs, return None. Otherwise return Some(physical_address).
pub fn virtual_to_physical(root: &PageTable, virtual_addr: usize) -> Option<usize> {
    let mut table = root;
    let mut current_level = PageEntryLevel::top();

    // Extract the parts of the VPN.
    let vpns = PageEntry::extract_vpns(virtual_addr);

    // Traverse the page table
    for vpn in vpns.into_iter().rev() {
        // A reference to the current entry that we're on (can be level 2, 1 or 0)
        let entry = &table.entries[vpn];

        match entry.get_type() {
            PageEntryType::Leaf => return Some(entry.get_ppn()),
            PageEntryType::Branch(next_addr) => {
                table = unsafe { (next_addr as *mut PageTable).as_mut().unwrap() };
            }
            PageEntryType::Invalid => todo!(),
        }

        current_level = match current_level.next_level() {
            Some(level) => level,
            None => panic!("There is no page size smaller than 4KiB"),
        }
    }
    // If we got to here, it means we havn't found a leaf.
    None
}

pub fn create_root_table() -> *mut PageTable {
    FRAMES_ALLOCATOR
        .lock()
        .zero_alloc(1, PageEntryLevel::KiB4)
        .cast::<PageTable>()
}
