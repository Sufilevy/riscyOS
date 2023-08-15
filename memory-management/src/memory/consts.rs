pub const FRAME_SIZE: usize = 0x1000;
pub const KIB: usize = 1024;
pub const NUM_VPNS: usize = 3;

pub const TEXT_START: usize = 0x0;
pub const TEXT_END: usize = 0x2000;
pub const RODATA_START: usize = 0x2000;
pub const RODATA_END: usize = 0x4000;
pub const DATA_START: usize = 0x4000;
pub const DATA_END: usize = 0x8000;
pub const BSS_START: usize = 0x8000;
pub const BSS_END: usize = 0x10000;
pub const STACK_START: usize = 0x10000;
pub const STACK_END: usize = 0x20000;
pub const HEAP_START: usize = 0x20000;
pub const HEAP_END: usize = 0x40000;
