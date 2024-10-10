//! The StackFrameAllocator allows the creation of "Frames"
//! where values can be pushed onto this frame.
//! Frames only exist in the scope they're created in using
//! the [new_frame](crate::stack_frame__allocator::StackFrameAllocator::new_frame)
//! function.  At the end of a frame's scope, the entire frame is popped,
//! and the StackFrameAllocator will continue pushing items
//! onto the previous frame.  Because only one StackRef can exist at a time
//! for a given value, both [get] and [get_mut] functions are safe,
//! due to being able to be validated by the borrow checker at compile time.

use std::{alloc::Layout, cell::UnsafeCell, fmt::Display, marker::PhantomData, ptr::NonNull};

use crate::{block_tail::BlockTail, stack_frame_header::StackFrameHeader, stack_ref::safe_ref::StackRef, stack_size::StackSize};

/// The StackFrameAllocator allows the creation of "Frames"
/// where key value pairs can be pushed onto this frame.
/// Frames only exist in the scope they're created in using
/// the [new_frame](crate::stack_frame_dict_allocator::StackFrameAllocator::new_frame)
/// function.  At the end of a frame's scope, the entire frame is popped,
/// and the StackFrameAllocator will continue pushing items
/// onto the previous frame.  Because only one StackRef can exist at a time
/// for a given value, both [get] and [get_mut] functions are safe,
/// due to being able to be validated by the borrow checker at compile time.
/// 
/// # Examples
/// 
/// ```edition2020
/// # use stack_frame_allocators::stack_frame_allocator::StackFrameAllocator;
/// 
/// let stack = StackFrameAllocator::<&str>::new();
/// stack.push("I");
/// stack.push("II");
/// stack.push("III");
/// 
/// stack.new_scope(|stack| {
///     let a = stack.push("a").get();
///     let mut b = stack.push("b").get_mut();
/// 
///     stack.new_scope(|stack| {
///         let one = stack.push("1").get();
///         let two = stack.push("2").get();
/// 
///         //this frame will pop here, 
///         //key values "1" and "2"
///         //are not reachable past this point
///     });
/// 
///     *b = "69";
/// 
///     //this frame will pop here, 
///     //key values "a" and "b"
///     //are not reachable past this point
/// });
/// ```
pub struct StackFrameAllocator<'s, Value> {
    pub(crate) size: StackSize,
    pub(crate) current_frame: UnsafeCell<NonNull<StackFrameHeader<'s>>>,
    pub(crate) buffer_bytes_used: UnsafeCell<usize>,
    pub(crate) phantom: PhantomData<Value>
}

impl<'s, Value> StackFrameAllocator<'s, Value> {
    const SIZE_HEADER:   usize = std::mem::size_of::<StackFrameHeader>();
    const SIZE_VALUE:    usize = std::mem::size_of::<Value>();
    const SIZE_TAIL:     usize = std::mem::size_of::<BlockTail>();

    const ALIGN_HEADER:     usize = std::mem::align_of::<StackFrameHeader>();
    const ALIGN_VALUE:      usize = std::mem::align_of::<Value>();
    #[allow(dead_code)]
    const ALIGN_TAIL:       usize = std::mem::align_of::<BlockTail>();

    /// Creates a new StackFrameAllocator
    /// 
    /// The StackFrameAllocator allows the creation of "Frames"
    /// where values can be pushed onto this frame.
    /// Frames only exist in the scope they're created in using
    /// the [new_frame](crate::stack_frame_allocator::StackFrameAllocator::new_frame)
    /// function.  At the end of a frame's scope, the entire frame is popped,
    /// and the StackFrameAllocator will continue pushing items
    /// onto the previous frame.
    /// 
    /// # Examples
    /// 
    /// ```edition2020
    /// # use stack_frame_allocators::stack_frame_allocator::StackFrameAllocator;
    /// 
    /// let stack = StackFrameAllocator::<&str>::new();
    /// stack.push("I");
    /// stack.push("II");
    /// stack.push("III");
    /// 
    /// stack.new_scope(|stack| {
    ///     let a = stack.push("a").get();
    ///     let mut b = stack.push("b").get_mut();
    /// 
    ///     stack.new_scope(|stack| {
    ///         let one = stack.push("1").get();
    ///         let two = stack.push("2").get();
    /// 
    ///         //this frame will pop here, 
    ///         //key values "1" and "2"
    ///         //are not reachable past this point
    ///     });
    /// 
    ///     *b = "69";
    /// 
    ///     //this frame will pop here, 
    ///     //key values "a" and "b"
    ///     //are not reachable past this point
    /// });
    /// ```
    pub fn new() -> Self {
        let size = StackSize::default();

        let allocated_block;
        let current_frame_pointer;
        unsafe {
            allocated_block = std::alloc::alloc(
                Layout::array::<u8>(size.bytes()).expect("could not allocate memory")
            );
            
            //size.bytes() should be a multiple of a large power of two,
            //therefore size.bytes() should be aligned to BlockTail already,
            //so we just need to move back so that way we're writing the block tail
            //at the end of the block
            let block_tail = allocated_block.add(size.bytes() - Self::SIZE_TAIL);
            (block_tail as *mut BlockTail).write(BlockTail {
                prev_block: std::ptr::null_mut(),
                prev_block_bytes_used: 0 /* we'll never read this value if prev_block is null */,
                next_block: std::ptr::null_mut()
            });

            current_frame_pointer = allocated_block.add(Self::SIZE_HEADER);
        }

        let init_frame = StackFrameHeader {
            previous_frame: None,
            current_frame_ptr: current_frame_pointer
        };

        unsafe {
            (allocated_block as *mut StackFrameHeader).write(init_frame) 
        };
        
        StackFrameAllocator {
            size,
            current_frame: UnsafeCell::new(unsafe {
                NonNull::new_unchecked(allocated_block as *mut StackFrameHeader)
            }),
            buffer_bytes_used: UnsafeCell::new(Self::SIZE_HEADER),
            phantom: PhantomData::default()
        }
    }

    /// Creates a new frame to push elements onto.
    /// 
    /// Creates a new scope where a new frame lives,
    /// at the end of the scope, the new frame and all its items
    /// will be popped.
    /// 
    /// It is good practice, whenever pushing items onto a stack allocator
    /// in a new scope, to instead create that scope using new_frame,
    /// since normally you can't access the values in the scope
    /// that were pushed onto the stack outside of that scope.
    /// It is still memory safe to use the Allocator in scopes,
    /// it is just not preffered.
    /// 
    /// Also its better to only have one instance of a frame.
    /// Creating multiple references to a stack can run into the same issue
    /// where you create values you, at some point, won't have access to.
    /// So functions using the Allocator should not take in references to it,
    /// and should instead create a new frame and pass-by-value.  
    /// It is still memory safe to pass references to the stack, 
    /// it is just not preferred.
    /// 
    /// # Examples
    /// 
    /// ```edition2020
    /// # use stack_frame_allocators::stack_frame_allocator::StackFrameAllocator;
    /// 
    /// pub fn bad_foo(stack: &StackFrameAllocator<&str>) {
    ///     //do stuff here
    /// }
    /// 
    /// pub fn good_foo(stack: StackFrameAllocator<&str>) {
    ///     //do stuff here
    /// }
    /// 
    /// # pub fn main() {
    /// let stack = StackFrameAllocator::<&str>::new();
    /// 
    /// //this is not preferred
    /// {
    ///     bad_foo(&stack);
    ///     
    ///     stack.push("no");
    ///     stack.push("non");
    /// }
    /// 
    /// //instead do this
    /// stack.new_scope(|stack| {
    ///     stack.new_scope(good_foo);
    ///     
    ///     stack.push("yes");
    ///     stack.push("oui");
    /// });
    /// # }
    /// ``` 
    pub fn new_scope<'n, F>(&self, mut scope: F) 
    where 
        's : 'n,
        F : FnMut(StackFrameAllocator<'n, Value>)
    {
        unsafe {
            let new_frame = StackFrameAllocator {
                size: self.size,
                current_frame: UnsafeCell::new((*self.current_frame.get()).clone()),
                buffer_bytes_used: UnsafeCell::new(
                    (*self.buffer_bytes_used.get()).clone()
                ),
                phantom: self.phantom
            };

            new_frame.generate_frame();

            //scope will automatically pop the new frame
            scope(new_frame);
        }
    }

    /// Creates a new frame to push elements onto within the same scope
    /// 
    /// [new_scope][stack_frame_allocators::stack_frame_allocator::StackFrameDictAllocator::new_scope]
    /// is generally preferred, however there are some use cases where you should be able to create
    /// a new frame and give ownership to it to a new scope.  This function is not recommended if you're
    /// not transferring ownership of the frame.  You also generally shouldn't push items onto the frame
    /// before transferring ownership, it is memory safe, but there's no logical purpose to it.  So a
    /// general rule of thumb is to never assign the return value to variable.
    /// 
    /// Also its better to only have one instance of a frame.
    /// Creating multiple references to a stack can run into the same issue
    /// where you create values you, at some point, won't have access to.
    /// So functions using the Allocator should not take in references to it,
    /// and should instead create a new frame and pass-by-value.  
    /// It is still memory safe to pass references to the stack, 
    /// it is just not preferred.
    /// 
    /// # Examples
    /// 
    /// ```edition2020
    /// # use stack_frame_allocators::stack_frame_allocator::StackFrameAllocator;
    /// 
    /// pub struct Chainable {
    ///     //input fields here
    /// }
    /// 
    /// impl Chainable {
    ///     pub fn chain(&self, stack: StackFrameAllocator<&str>, input: usize) -> Chainable {
    ///         //do stuff
    ///         # return Chainable {};
    ///     }
    /// }
    /// 
    /// # pub fn main() {
    /// let stack = StackFrameAllocator::<&str>::new();
    /// 
    /// let chain = Chainable { /* assign fields */ };
    /// 
    /// chain.chain(stack.new_frame(), 1)
    ///      .chain(stack.new_frame(), 2)
    ///      .chain(stack.new_frame(), 3);
    /// # }
    /// ```
    pub fn new_frame(&self) -> StackFrameAllocator<'s, Value> {
        let stack;
        unsafe {
            stack = StackFrameAllocator {
                size: self.size,
                current_frame: UnsafeCell::new((*self.current_frame.get()).clone()),
                buffer_bytes_used: UnsafeCell::new(
                    (*self.buffer_bytes_used.get()).clone()
                ),
                phantom: self.phantom
            };

            stack.generate_frame();
        }

        return stack;
    }

    unsafe fn generate_frame<'n>(&self) {
        let header_padding = (*(*self.current_frame.get()).as_ptr())
            .current_frame_ptr
            .align_offset(Self::ALIGN_HEADER);
        let can_push_to_block = *self.buffer_bytes_used.get() + 
            header_padding + Self::SIZE_HEADER < 
            self.real_size().bytes();
        
        let mem = if can_push_to_block {
            *self.buffer_bytes_used.get() += header_padding + Self::SIZE_HEADER;

            (*(*self.current_frame.get()).as_ptr())
                .current_frame_ptr
                .add(header_padding + Self::SIZE_HEADER)
        } else {
            let curr_block_tail = self.get_block_tail();
            
            if curr_block_tail.next_block.is_null() {
                let allocated_block = unsafe {std::alloc::alloc(
                    Layout::array::<u8>(self.size.bytes())
                        .expect("could not allocate memory")
                )};

                let next_block_tail = allocated_block.add(
                    self.size.bytes() - Self::SIZE_TAIL
                );
                //eprintln!("writing block tail at {:?}", next_block_tail);
                (next_block_tail as *mut BlockTail).write(BlockTail {
                    prev_block: (*self.current_frame.get()).as_ptr().cast(),
                    prev_block_bytes_used: (*self.buffer_bytes_used.get()),
                    next_block: std::ptr::null_mut()
                });

                curr_block_tail.next_block = allocated_block;
            }

            curr_block_tail.next_block
        };

        let current_frame_ptr = mem.add(Self::SIZE_HEADER);
        
        let new_frame = StackFrameHeader {
            previous_frame: Some((*self.current_frame.get()).as_ref()),
            current_frame_ptr
        };

        (mem as *mut StackFrameHeader).write(new_frame);

        *self.current_frame.get() = NonNull::new_unchecked(mem as *mut StackFrameHeader);
    }

    /// The Tail End of a Memory Block is reserved for storing
    /// the address to the previous block, 
    /// how many bytes of the previous block is used,
    /// and the address to the next block.
    /// This Tail effectively reduces the usable size of the block
    /// 
    /// # Examples
    /// A memory block with layout
    /// ```text
    ///   0x0000_0000_0000_0001
    ///   0x0000_0000_0000_0002
    ///   0x0000_0000_0000_0003
    ///   0x0000_0000_0000_0004
    ///   0x0000_0000_0000_0005
    ///   0x0000_aaaa_aaaa_aaa0 <- address to prev block
    ///   0x0000_0000_0000_0400 <- bytes used of prev block
    ///   0x0000_ffff_ffff_fff0 <- address to next block
    /// ```
    /// has size 8 words, however 3 words are reserved
    /// so `real_size(&self)` will return 5 words worth of space
    #[inline]
    fn real_size(&self) -> StackSize {
        StackSize(self.size.bytes() - Self::SIZE_TAIL)
    }

    unsafe fn get_block_tail(&self) -> &mut BlockTail {
        let offset = self.real_size().bytes() - *self.buffer_bytes_used.get();
        
        return (*self.current_frame.get())
            .as_ref()
            .current_frame_ptr
            .add(offset)
            .cast::<BlockTail>()
            .as_mut()
            .expect("Error grabbing mutable reference to BlockTail");
    }

    /// Pushes a Value into the current frame,
    /// returning a StackRef to the Value.
    /// 
    /// # Examples
    /// 
    /// ```edition2020
    /// # use stack_frame_allocators::stack_frame_allocator::StackFrameAllocator;
    /// 
    /// let stack = StackFrameAllocator::<usize>::new();
    /// 
    /// let a = stack.push(1).get();
    /// let b = stack.push(2).get();
    /// 
    /// assert_eq!(*a, 1);
    /// assert_eq!(*b, 2);
    /// ```
    pub fn push<'a>(
        &'a self, 
        value: Value
    ) -> StackRef<'a, Value> {
        let (value_padding, can_push_to_block, current_frame_ptr);
        let value_ptr: *mut u8;
        
        unsafe {
            current_frame_ptr = (*(*self.current_frame.get()).as_ptr())
                .current_frame_ptr;
            value_padding = current_frame_ptr
                .align_offset(Self::ALIGN_VALUE);
            value_ptr = current_frame_ptr.add(value_padding);
            can_push_to_block = *self.buffer_bytes_used.get() + 
                value_padding + Self::SIZE_VALUE < 
                self.real_size().bytes();
        }
        
        if can_push_to_block { unsafe {
            // eprintln!("writing key of size {} at {:?} with {}",
            //     Self::SIZE_KEY, key_ptr, &key
            // );
            // eprintln!("writing key of size {} at {:?} with {}",
            //     Self::SIZE_VALUE, value_ptr, &value
            // );
            (value_ptr as *mut Value).write(value);
            let offset = value_padding + Self::SIZE_VALUE;
            (*(*self.current_frame.get()).as_ptr()).current_frame_ptr = {
                current_frame_ptr.add(offset)
            };

            *self.buffer_bytes_used.get() += offset;

            return StackRef {
                value: value_ptr as *mut Value,
                phantom: PhantomData::default()
            };
        }} else { unsafe {
            let curr_block_tail = self.get_block_tail();
            
            //if there is no next block, create one
            if curr_block_tail.next_block.is_null() {
                let allocated_block = std::alloc::alloc(
                    Layout::array::<u8>(self.size.bytes())
                        .expect("could not allocate memory")
                );

                let next_block_tail = allocated_block
                    .add(self.size.bytes() - Self::SIZE_TAIL);
                //eprintln!("writing block tail at {:?}", next_block_tail);
                (next_block_tail as *mut BlockTail).write(BlockTail {
                    prev_block: (*self.current_frame.get()).as_ref().current_frame_ptr,
                    prev_block_bytes_used: (*self.buffer_bytes_used.get()),
                    next_block: std::ptr::null_mut()
                });

                curr_block_tail.next_block = allocated_block;
            }

            let next_block_addr_ptr = curr_block_tail.next_block;
            //value_padding is not needed, 
            //because the block should already be aligned to Key, 
            //but its added for consistency
            let value_padding = next_block_addr_ptr
                .align_offset(Self::ALIGN_VALUE);
            let value_ptr = next_block_addr_ptr.add(value_padding);

            let block_offset = value_padding + Self::SIZE_VALUE;

            *self.buffer_bytes_used.get() = block_offset;

            // eprintln!("writing key of size {} at {:?} with {}",
            //     Self::SIZE_KEY, key_ptr, &key
            // );
            // eprintln!("writing key of size {} at {:?} with {}",
            //     Self::SIZE_VALUE, value_ptr, &value
            // );

            (value_ptr as *mut Value).write(value);
            (*(*self.current_frame.get()).as_ptr()).current_frame_ptr =
                next_block_addr_ptr.add(block_offset);

            return StackRef {
                value: value_ptr as *mut Value,
                phantom: PhantomData::default()
            };
        }}
    }

    /// prints out the current stack from last push (top) to first push (bottom)
    /// 
    /// Includes where headers are.
    /// 
    /// # Examples
    /// 
    /// ```edition2020
    /// # use stack_frame_allocators::stack_frame_allocator::StackFrameAllocator;
    /// 
    /// let stack = StackFrameAllocator::<usize>::new();
    /// stack.push(1);
    /// stack.push(2);
    /// stack.push(3);
    /// stack.print();
    /// 
    /// stack.new_scope(|stack| {
    ///     stack.push(10);
    ///     stack.push(20);
    ///     stack.push(30);
    ///     stack.print();
    /// 
    ///     stack.new_scope(|stack| {
    ///         stack.push(100);
    ///         stack.push(200);
    ///         stack.print();
    ///     });
    /// 
    ///     stack.push(40);
    ///     stack.print();
    /// });
    /// 
    /// stack.push(4);
    /// stack.push(5);
    /// stack.print();
    /// ```
    /// 
    /// Will print out:
    /// ```text
    /// First print!
    /// 
    /// top of stack
    ///     3
    ///     2
    ///     1
    /// header
    /// 
    /// Second print!
    /// 
    /// top of stack
    ///     30
    ///     20
    ///     10
    /// header
    ///     3
    ///     2
    ///     1
    /// header
    /// 
    /// Third print!
    /// 
    /// top of stack
    ///     200
    ///     100
    /// header
    ///     30
    ///     20
    ///     10
    /// header
    ///     3
    ///     2
    ///     1
    /// header
    /// 
    /// Fourth print!
    /// 
    /// top of stack
    ///     40
    ///     30
    ///     20
    ///     10
    /// header
    ///     3
    ///     2
    ///     1
    /// header
    /// 
    /// Fifth print!
    /// 
    /// top of stack
    ///     5
    ///     4
    ///     3
    ///     2
    ///     1
    /// header
    /// ```
    pub fn print(&self) where Value: Display {
        let mut count_blocks = 1;

        let mut curr_block_tail = unsafe {self.get_block_tail()};
        let mut bytes_remaining = unsafe {*self.buffer_bytes_used.get()};

        let mut stack_frame = unsafe {(*self.current_frame.get()).as_ref()};
        let mut peek_ptr = stack_frame.current_frame_ptr;
        
        //for the first scope we're looking at, because it's the newest scope
        //there should be no headers after the current frame,
        //so we'll use key alignment
        let mut just_jumped_block = false;
        let mut expect_key_value_pair = true;
        let mut stack_frame_ptr_after = {unsafe {
            let offset_ptr = (stack_frame as *const StackFrameHeader as *mut u8)
                .add(Self::SIZE_HEADER);
            let padding = offset_ptr.align_offset(Self::ALIGN_VALUE);
            offset_ptr.add(padding)
        }};

        println!("top of stack");

        loop {unsafe {
            if bytes_remaining == 0 {
                if curr_block_tail.prev_block.is_null() {
                    unreachable!("{}", concat!(
                        "the previous block can only be null ",  
                        "if the block currently being looked at is the first block.  ",  
                        "In that case, the header logic would've ran first, ", 
                        "thus this should never be reached"
                    ))
                }

                count_blocks += 1;

                bytes_remaining = curr_block_tail.prev_block_bytes_used;
                peek_ptr = curr_block_tail.prev_block;

                let offset = self.real_size().bytes() - bytes_remaining;
        
                curr_block_tail = peek_ptr
                    .add(offset)
                    .cast::<BlockTail>()
                    .as_mut()
                    .expect("Error grabbing mutable reference to BlockTail");

                //we must check for the case, the first key value pair attached
                //to this header was in the block we were just looking in
                //in this case, there should be no padding
                stack_frame_ptr_after = (
                    stack_frame 
                    as *const StackFrameHeader 
                    as *mut u8
                ).add(Self::SIZE_HEADER);

                just_jumped_block = true;
            }
            
            if peek_ptr < stack_frame_ptr_after {
                unreachable!("unexpected operation caused peek_ptr to go past the stack_frame_ptr");
            } else if peek_ptr == stack_frame_ptr_after {
                println!("header");

                let Some(new_frame) = stack_frame.previous_frame else {
                    break;
                };

                stack_frame = new_frame;
                peek_ptr = stack_frame.current_frame_ptr;

                //this new header could have zero items
                just_jumped_block = false;
                expect_key_value_pair = false;
                stack_frame_ptr_after = {
                    let offset_ptr = (stack_frame as *const StackFrameHeader as *mut u8)
                        .add(Self::SIZE_HEADER);
                    let padding = offset_ptr.align_offset(Self::ALIGN_HEADER);
                    offset_ptr.add(padding)
                };

                continue;
            } else if !expect_key_value_pair || just_jumped_block {
                just_jumped_block = false;
                expect_key_value_pair = true;

                stack_frame_ptr_after = {
                    let offset_ptr = (stack_frame as *const StackFrameHeader as *mut u8)
                        .add(Self::SIZE_HEADER);
                    let padding = offset_ptr.align_offset(Self::ALIGN_VALUE);
                    offset_ptr.add(padding)
                };
            }
            
            peek_ptr = peek_ptr.sub(Self::SIZE_VALUE);
            let value = peek_ptr.cast::<Value>().as_ref().unwrap_unchecked();
            println!("\t{}", value);
        }}

        println!("\n{} block(s) of size {} bytes have been allocated.\n", 
            count_blocks, 
            self.size.bytes()
        );
    }

    //TODO add allocated_blocks(&self) -> usize and using_blocks(&self) -> usize functions
}

impl<'s, Value> Drop for StackFrameAllocator<'s, Value> {
    fn drop(&mut self) {
        //eprintln!("dropping stack frame");
        unsafe {
            let current_frame_ptr = (*self.current_frame.get()).as_ptr().cast::<u8>();
            let mut bytes_remaining = *self.buffer_bytes_used.get();
            let mut peek_ptr = (*current_frame_ptr.cast::<StackFrameHeader>()).current_frame_ptr;
            let mut curr_block_tail = self.get_block_tail();
    
            //because we're only dropping the current scope,
            //we can assume the padding after the header
            //is key padding, because we shouldn't be expecting a header 
            //after the header we're looking in
            let stack_frame_ptr_after = {
                let offset_ptr = current_frame_ptr.add(Self::SIZE_HEADER);
                let padding = offset_ptr.align_offset(Self::ALIGN_VALUE);
                offset_ptr.add(padding)
            };
    
            //eprintln!("starting search at {:?} until {:?}", peek_ptr, stack_frame_ptr_after);
            while peek_ptr > stack_frame_ptr_after {
                // eprintln!("peeking at {:?} until {:?} with {} bytes remaining", 
                //     peek_ptr, stack_frame_ptr_after, bytes_remaining
                // );
                if bytes_remaining == 0 {
                    if curr_block_tail.prev_block.is_null() {
                        unreachable!("{}", concat!(
                            "the previous block can only be null ",  
                            "if the block currently being looked at is the first block.  ",  
                            "In that case, the header logic would've ran first, ", 
                            "thus this should never be reached"
                        ))
                    }
                    bytes_remaining = curr_block_tail.prev_block_bytes_used;
                    peek_ptr = curr_block_tail.prev_block;
    
                    let offset = self.real_size().bytes() - bytes_remaining;
            
                    curr_block_tail = peek_ptr
                        .add(offset)
                        .cast::<BlockTail>()
                        .as_mut()
                        .expect("Error grabbing mutable reference to BlockTail");
                }
    
                //dropping key and value pair
                peek_ptr = peek_ptr.sub(Self::SIZE_VALUE);
                bytes_remaining -= Self::SIZE_VALUE;
                
                std::ptr::drop_in_place(peek_ptr.cast::<Value>());
            }
            
            if (*self.current_frame.get()).as_ref().previous_frame.is_none() {
                //eprintln!("dropping whole stack");
                let mut prev_addr;
                let mut next_addr = (*self.current_frame.get()).as_ptr() as *mut u8;

                while !next_addr.is_null() {
                    //eprintln!("dropping block of size {} bytes at {:?}", self.size.bytes(), next_addr);
                    
                    prev_addr = next_addr;
                    //eprintln!("grabbing tail at {:?}", next_addr.add(self.real_size().bytes()));
                    let block_tail = next_addr.add(self.real_size().bytes())
                        .cast::<BlockTail>().as_ref().unwrap_unchecked();
                    //eprintln!("successfully grabbed tail");
                    next_addr = block_tail.next_block;

                    std::alloc::dealloc(prev_addr, Layout::array::<u8>(self.size.bytes()).expect("fuck"));
                }
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use std::cell::RefCell;

    #[doc(hidden)]
    pub struct DropTest<'d>(&'d str, &'d RefCell<Vec<&'d str>>);

    impl<'d> Drop for DropTest<'d> {
        fn drop(&mut self) {
            let (value, dropped) = (self.0, self.1);

            dropped.borrow_mut().push(value);
        }
    }

    #[doc(hidden)]
    #[derive(PartialEq, Eq, Hash)]
    pub struct DropPrint<T>(T) where T : Display;

    impl<T> Drop for DropPrint<T> where T : Display {
        fn drop(&mut self) {
            println!("{}", self.0);
        }
    }

    #[test]
    pub fn drop_scope_test() {
        let dropped = RefCell::new(vec![]);
        {
            let stack = StackFrameAllocator::<DropTest>::new();
            stack.push(DropTest("value1scope1", &dropped));
            stack.push(DropTest("value2scope1", &dropped));
            stack.push(DropTest("value3scope1", &dropped));
            stack.new_scope(|stack| {
                stack.push(DropTest("value1scope2", &dropped));
                stack.push(DropTest("value2scope2", &dropped));
                stack.push(DropTest("value3scope2", &dropped));
            });
        }

        let compare = vec![
            "value3scope2", 
            "value2scope2", 
            "value1scope2", 
            "value3scope1", 
            "value2scope1", 
            "value1scope1"
        ];

        assert_eq!(*dropped.borrow(), compare);
    }

    #[test]
    pub fn drop_frame_test() {
        let dropped = RefCell::new(vec![]);
        {
            let stack = StackFrameAllocator::<DropTest>::new();
            stack.push(DropTest("value1scope1", &dropped));
            stack.push(DropTest("value2scope1", &dropped));
            stack.push(DropTest("value3scope1", &dropped));
            let stack = stack.new_frame();
            stack.push(DropTest("value1scope2", &dropped));
            stack.push(DropTest("value2scope2", &dropped));
            stack.push(DropTest("value3scope2", &dropped));
        }
    }

    #[test]
    #[allow(unused_variables)]
    pub fn empty_drop_test() {
        let stack_u8 = StackFrameAllocator::<DropPrint<u8>>::new();
        let stack_u16 = StackFrameAllocator::<DropPrint<u16>>::new();
        let stack_u32 = StackFrameAllocator::<DropPrint<u32>>::new();
        let stack_u64 = StackFrameAllocator::<DropPrint<u64>>::new();
        let stack_u128 = StackFrameAllocator::<DropPrint<u128>>::new();
    }
}