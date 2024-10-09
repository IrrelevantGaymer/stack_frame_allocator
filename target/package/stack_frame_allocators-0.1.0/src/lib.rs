#![warn(missing_docs)]
#![feature(ptr_as_ref_unchecked)]

//! A set of Allocators based on the concept of a stack
//! and enforcing memory safety via scopes.  These allocators
//! use frames, where values can be pushed onto the frames.
//! The allocators can only pop whole frames and all of its
//! associated values.

pub(crate) mod block_tail;
pub mod stack_frame_allocator;
pub mod stack_frame_dict_allocator;
pub(crate) mod stack_frame_header;
pub mod stack_ref;
pub(crate) mod stack_size;