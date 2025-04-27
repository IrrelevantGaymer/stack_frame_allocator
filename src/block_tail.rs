/// A block of data placed at the end of memory blocks 
/// to keep track of allocated blocks and help
/// traverse the stack
#[derive(Debug)]
pub(crate) struct BlockTail {
    pub(crate) prev_block: *mut u8,
    prev_block_bytes_used: usize,
    pub(crate) next_block: *mut u8
}

impl BlockTail {
    #[inline]
    pub(crate) fn new(
        prev_block: *mut u8, 
        prev_block_bytes_used: usize, 
        next_block: *mut u8
    ) -> Self {
        BlockTail { prev_block, prev_block_bytes_used, next_block }
    }
    
    #[inline(always)]
    pub(crate) fn prev_block_bytes_used(&self) -> usize {
        self.prev_block_bytes_used
    }
}

impl Default for BlockTail {
    #[inline]
    fn default() -> Self {
        BlockTail { 
            prev_block: std::ptr::null_mut(), 
            prev_block_bytes_used: 0, 
            next_block: std::ptr::null_mut() 
        }
    }
}