/// A block of data placed in memory blocks to keep track of active stack frames
/// and help traverse the stack
pub(crate) struct StackFrameHeader<'sf> {
    pub(crate) previous_frame: Option<&'sf StackFrameHeader<'sf>>,
    pub(crate) current_frame_ptr: *mut u8
}