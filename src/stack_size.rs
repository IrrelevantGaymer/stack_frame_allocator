#[allow(missing_docs)]

#[derive(Clone, Copy)]
pub(crate) struct StackSize(pub(crate) usize);

impl StackSize {
    pub fn from_num_bytes(bytes: usize) -> Self {
        StackSize(bytes * std::mem::size_of::<u8>())
    }

    pub fn bytes(self) -> usize {
        self.0
    }
}

impl Default for StackSize {
    fn default() -> Self {
        StackSize::from_num_bytes(1024)
    }
}