/// A block of data placed at the end of memory blocks 
/// to keep track of allocated blocks and help
/// traverse the stack
pub(crate) struct BlockTail {
    pub(crate) prev_block: *mut u8,
    pub(crate) prev_block_bytes_used: usize,
    pub(crate) next_block: *mut u8
}