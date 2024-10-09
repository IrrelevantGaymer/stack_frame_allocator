//! The StackFrameDictAllocator allows the creation of "Frames"
//! where key value pairs can be pushed onto this frame.
//! Frames only exist in the scope they're created in using
//! the [new_frame](crate::stack_frame_dict_allocator::StackFrameDictAllocator::new_frame)
//! function.  At the end of a frame's scope, the entire frame is popped,
//! and the StackFrameDictAllocator will continue pushing items
//! onto the previous frame.  Key Value pairs can be grabbed by 
//! searching for the last entry with that key.

use std::{alloc::Layout, cell::UnsafeCell, fmt::Display, hash::Hash, marker::PhantomData, ptr::NonNull};

use crate::{block_tail::BlockTail, stack_frame_header::StackFrameHeader, stack_ref::unsafe_ref::StackRef, stack_size::StackSize};

/// The StackFrameDictAllocator allows the creation of "Frames"
/// where key value pairs can be pushed onto this frame.
/// Frames only exist in the scope they're created in using
/// the [new_frame](crate::stack_frame_dict_allocator::StackFrameDictAllocator::new_frame)
/// function.  At the end of a frame's scope, the entire frame is popped,
/// and the StackFrameDictAllocator will continue pushing items
/// onto the previous frame.  Key Value pairs can be grabbed by 
/// searching for the last entry with that key.
/// 
/// # Examples
/// 
/// ```edition2020
/// # use stack_frame_allocator::stack_frame_dict_allocator::StackFrameDictAllocator;
/// 
/// use std::cell::RefCell;
/// 
/// let stack = StackFrameDictAllocator::<&str, RefCell<usize>>::new();
/// stack.push("I", RefCell::new(0));
/// stack.push("II", RefCell::new(1));
/// stack.push("III", RefCell::new(2));
/// 
/// stack.new_frame(|stack| {
///     stack.push("a", RefCell::new(3));
///     stack.push("b", RefCell::new(4));
/// 
///     stack.new_frame(|stack| {
///         stack.push("1", RefCell::new(5));
///         stack.push("2", RefCell::new(6));
/// 
///         //this frame will pop here, 
///         //key values ("1", RefCell(5)) and ("2", RefCell(6))
///         //are not reachable past this point
///     });
/// 
///     let mut b = stack.get_in_frame("b").unwrap().get().borrow_mut();
///     *b = 69;
/// 
///     //this frame will pop here, 
///     //key values ("a", RefCell(3)) and ("b", RefCell(69))
///     //are not reachable past this point
/// });
/// ```
pub struct StackFrameDictAllocator<'s, Key, Value> 
where 
    Key: Eq + Hash
{
    pub(crate) size: StackSize,
    pub(crate) current_frame: UnsafeCell<NonNull<StackFrameHeader<'s>>>,
    pub(crate) buffer_bytes_used: UnsafeCell<usize>,
    pub(crate) phantom: PhantomData<(Key, Value)>
}

impl<'s, Key, Value> StackFrameDictAllocator<'s, Key, Value> 
where 
    Key: Eq + Hash
{
    const SIZE_HEADER:   usize = std::mem::size_of::<StackFrameHeader>();
    const SIZE_KEY:      usize = std::mem::size_of::<Key>();
    const SIZE_VALUE:    usize = std::mem::size_of::<Value>();
    const SIZE_TAIL:     usize = std::mem::size_of::<BlockTail>();

    const ALIGN_HEADER:     usize = std::mem::align_of::<StackFrameHeader>();
    const ALIGN_KEY:        usize = std::mem::align_of::<Key>();
    const ALIGN_VALUE:      usize = std::mem::align_of::<Value>();
    #[allow(dead_code)]
    const ALIGN_TAIL:       usize = std::mem::align_of::<BlockTail>();

    /// Creates a new StackFrameDictAllocator
    /// 
    /// The StackFrameDictAllocator allows the creation of "Frames"
    /// where key value pairs can be pushed onto this frame.
    /// Frames only exist in the scope they're created in using
    /// the [new_frame](crate::stack_frame_dict_allocator::StackFrameDictAllocator::new_frame)
    /// function.  At the end of a frame's scope, the entire frame is popped,
    /// and the StackFrameDictAllocator will continue pushing items
    /// onto the previous frame.  Key Value pairs can be grabbed by 
    /// searching for the last entry with that key.
    /// 
    /// # Examples
    /// 
    /// ```edition2020
    /// # use stack_frame_allocator::stack_frame_dict_allocator::StackFrameDictAllocator;
    /// 
    /// use std::cell::RefCell;
    /// 
    /// let stack = StackFrameDictAllocator::<&str, RefCell<usize>>::new();
    /// stack.push("I", RefCell::new(0));
    /// stack.push("II", RefCell::new(1));
    /// stack.push("III", RefCell::new(2));
    /// 
    /// stack.new_frame(|stack| {
    ///     stack.push("a", RefCell::new(3));
    ///     stack.push("b", RefCell::new(4));
    /// 
    ///     stack.new_frame(|stack| {
    ///         stack.push("1", RefCell::new(5));
    ///         stack.push("2", RefCell::new(6));
    /// 
    ///         //this frame will pop here, 
    ///         //key values ("1", RefCell(5)) and ("2", RefCell(6))
    ///         //are not reachable past this point
    ///     });
    /// 
    ///     let mut b = stack.get_in_frame("b").unwrap().get().borrow_mut();
    ///     *b = 69;
    /// 
    ///     //this frame will pop here, 
    ///     //key values ("a", RefCell(3)) and ("b", RefCell(69))
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
        
        StackFrameDictAllocator {
            size,
            current_frame: UnsafeCell::new(unsafe {
                NonNull::new_unchecked(allocated_block as *mut StackFrameHeader)
            }),
            buffer_bytes_used: UnsafeCell::new(Self::SIZE_HEADER),
            phantom: PhantomData::default()
        }
    }

    /// Creates a new frame to push elements onto in a new scope.
    /// 
    /// Creates a new scope where a new frame lives,
    /// at the end of the scope, the new frame and all its items
    /// will be popped.
    /// 
    /// It is good practice, whenever pushing items onto a stack allocator
    /// in a new scope, to instead create that scope using new_frame,
    /// since normally you can't access the values in the scope
    /// that were pushed onto the stack outside of that scope.
    /// In the case of the StackFrameDictAllocator where you theoretically
    /// could access these values outside the scope they're defined in
    /// using the [get_in_frame](crate::stack_frame_dict_allocator::StackFrameDictAllocator::get_in_frame)
    /// function, it makes no sense to structure your code this way.
    /// It is still memory safe to use the Allocator in scopes without using
    /// [get_in_frame](crate::stack_frame_dict_allocator::StackFrameDictAllocator::get_in_frame),
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
    /// # use stack_frame_allocator::stack_frame_dict_allocator::StackFrameDictAllocator;
    /// 
    /// pub fn bad_foo(stack: &StackFrameDictAllocator<&str, usize>) {
    ///     //do stuff here
    /// }
    /// 
    /// pub fn good_foo(stack: StackFrameDictAllocator<&str, usize>) {
    ///     //do stuff here
    /// }
    /// 
    /// # pub fn main() {
    /// let stack = StackFrameDictAllocator::<&str, usize>::new();
    /// 
    /// //this is not preferred
    /// {
    ///     bad_foo(&stack);
    ///     
    ///     stack.push("no", 240);
    ///     stack.push("non", 96);
    /// }
    /// 
    /// //instead do this
    /// stack.new_frame(|stack| {
    ///     stack.new_frame(good_foo);
    ///     
    ///     stack.push("yes", 420);
    ///     stack.push("oui", 69);
    /// });
    /// # }
    /// ``` 
    pub fn new_scope<'n, F>(&self, mut scope: F) 
    where 
        's : 'n,
        Key : 'n, 
        F : FnMut(StackFrameDictAllocator<'n, Key, Value>)
    {
        unsafe {
            let new_frame = StackFrameDictAllocator {
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
    /// [new_scope][stack_frame_allocators::stack_frame_dict_allocator::StackFrameDictAllocator::new_scope]
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
    /// pub struct Chainable {
    ///     //input fields here
    /// };
    /// 
    /// impl Chainable {
    ///     pub fn chain(&self, stack: StackFrameDictAllocator<&str, usize>, input: usize) -> Chainable {
    ///         //do stuff
    ///     }
    /// }
    /// 
    /// #pub fn main() {
    /// let stack = StackFrameDictAllocator::<&str, usize>::new();
    /// 
    /// let chain = Chainable { /* assign fields */ };
    /// 
    /// chain.chain(stack.new_frame(), 1)
    ///      .chain(stack.new_frame(), 2)
    ///      .chain(stack.new_frame(), 3);
    /// #}
    /// ```
    pub fn new_frame(&self) -> StackFrameDictAllocator<'s, Key, Value> {
        unsafe {StackFrameDictAllocator {
            size: self.size,
            current_frame: UnsafeCell::new((*self.current_frame.get()).clone()),
            buffer_bytes_used: UnsafeCell::new(
                (*self.buffer_bytes_used.get()).clone()
            ),
            phantom: self.phantom
        }}
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

    /// Pushes a Key Value pair into the current frame,
    /// returning a StackRef to the Value.
    /// 
    /// Multiple Values can have the same key, even within the same frame.
    /// This allows for shadowing, such that when using 
    /// [get_in_frame](crate::stack_frame_dict_allocator::StackFrameDictAllocator::get_in_frame)
    /// you grab the last value that was pushed with that key.
    /// For example, I add the pair ("key", "first") and then the pair ("key", "second").
    /// calling `stack.get_in_frame("key")` will grab ("key", "second"), not ("key", "first"),
    /// because ("key", "second") shadows ("key", "first")
    /// 
    /// # Examples
    /// 
    /// ```edition2020
    /// use std::cell::RefCell;
    /// # use stack_frame_allocator::stack_frame_dict_allocator::StackFrameDictAllocator;
    /// 
    /// let stack = StackFrameDictAllocator::<&str, RefCell<usize>>::new();
    /// 
    /// {
    ///     let mut a = stack.push("a", RefCell::new(0)).get().borrow_mut();
    ///     
    ///     assert_eq!(*a, 0);
    ///     *a += 2;
    ///     assert_eq!(*a, 2);
    /// }
    /// 
    /// {
    ///     let mut a = stack.push("a", RefCell::new(10)).get().borrow_mut();
    ///     
    ///     assert_eq!(*a, 10);
    ///     *a -= 5;
    ///     assert_eq!(*a, 5);
    /// }
    /// 
    /// //grabs the newest value with the key "a"
    /// {
    ///     let mut a = stack.get_in_frame("a").unwrap().get().borrow_mut();
    ///     
    ///     assert_eq!(*a, 5);
    ///     *a += 5;
    ///     assert_eq!(*a, 10);
    /// }
    /// 
    /// ```
    pub fn push<'a>(
        &'a self, 
        key: impl Into<Key>, 
        value: Value
    ) -> StackRef<'a, Value> {
        let (key_padding, value_padding, can_push_to_block, current_frame_ptr);
        let (key_ptr, value_ptr): (*mut u8, *mut u8);
        
        unsafe {
            current_frame_ptr = (*(*self.current_frame.get()).as_ptr())
                .current_frame_ptr;
            key_padding = current_frame_ptr.align_offset(Self::ALIGN_KEY);
            key_ptr = current_frame_ptr.add(key_padding);
            value_padding = key_ptr
                .add(Self::SIZE_KEY)
                .align_offset(Self::ALIGN_VALUE);
            value_ptr = key_ptr.add(Self::SIZE_KEY + value_padding);
            can_push_to_block = *self.buffer_bytes_used.get() + 
                key_padding + Self::SIZE_KEY + 
                value_padding + Self::SIZE_VALUE < 
                self.real_size().bytes();
        }
        
        if can_push_to_block { unsafe {
            let key = key.into();
            // eprintln!("writing key of size {} at {:?} with {}",
            //     Self::SIZE_KEY, key_ptr, &key
            // );
            // eprintln!("writing key of size {} at {:?} with {}",
            //     Self::SIZE_VALUE, value_ptr, &value
            // );
            (key_ptr as *mut Key).write(key);
            (value_ptr as *mut Value).write(value);
            let offset = key_padding + Self::SIZE_KEY + 
                value_padding + Self::SIZE_VALUE;
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
            //key_padding is not needed, 
            //because the block should already be aligned to Key, 
            //but its added for consistency
            let key_padding = next_block_addr_ptr
                .align_offset(Self::ALIGN_KEY);
            let key_ptr = next_block_addr_ptr.add(key_padding);
            let value_padding = key_ptr
                .add(Self::SIZE_KEY)
                .align_offset(Self::ALIGN_VALUE);
            let value_ptr = key_ptr.add(Self::SIZE_KEY + value_padding);

            let block_offset = key_padding + Self::SIZE_KEY +
                value_padding + Self::SIZE_VALUE;

            *self.buffer_bytes_used.get() = block_offset;

            let key = key.into();
            // eprintln!("writing key of size {} at {:?} with {}",
            //     Self::SIZE_KEY, key_ptr, &key
            // );
            // eprintln!("writing key of size {} at {:?} with {}",
            //     Self::SIZE_VALUE, value_ptr, &value
            // );

            (key_ptr as *mut Key).write(key.into());
            (value_ptr as *mut Value).write(value);
            (*(*self.current_frame.get()).as_ptr()).current_frame_ptr =
                next_block_addr_ptr.add(block_offset);

            return StackRef {
                value: value_ptr as *mut Value,
                phantom: PhantomData::default()
            };
        }}
    }

    /// Finds the latest Value with that Key in the current Frame, returning a StackRef to it.
    ///
    /// Allows you to dynamically grab values pushed into a frame
    /// by searching for its key.  Multiple Values can have the same key,
    /// so pushing a Value with a Key already used, 
    /// shadows the previous Value with that Key.  This function
    /// finds the last Value with that Key, so Values that are currently being shadowed,
    /// cannot be found by this function.  This function also only searches
    /// the current Frame, so Values pushed before this current Frame cannot
    /// be found with function.  If no Value contains this Key in the current Frame,
    /// this function will return a None.
    /// 
    /// # Examples
    /// 
    /// ```edition2020
    /// # use stack_frame_allocator::stack_frame_dict_allocator::StackFrameDictAllocator;
    /// 
    /// let stack = StackFrameDictAllocator::<&str, &str>::new();
    /// 
    /// stack.push("red", "first");
    /// stack.push("blue", "first");
    /// 
    /// stack.new_frame(|stack| {
    ///     stack.push("red", "second");
    /// 
    ///     let red = stack.get_in_frame("red").unwrap().get();
    ///     let blue = stack.get_in_frame("blue");
    ///     assert_eq!(*red, "second");
    ///     assert!(blue.is_none());
    /// });
    /// 
    /// let red = stack.get_in_frame("red").unwrap().get();
    /// let blue = stack.get_in_frame("blue").unwrap().get();
    /// assert_eq!(*red, "first");
    /// assert_eq!(*blue, "first");
    /// 
    /// //shadow blue
    /// stack.push("blue", "second");
    /// 
    /// let blue = stack.get_in_frame("blue").unwrap().get();
    /// assert_eq!(*blue, "second");
    /// ```
    pub fn get_in_frame<'a>(
        &'a self, 
        key: impl Into<Key>
    ) -> Option<StackRef<'a, Value>> {
        let key = key.into();

        let current_frame_ptr = unsafe {
            (*self.current_frame.get()).as_ptr().cast::<u8>()
        };
        let mut bytes_remaining = unsafe {*self.buffer_bytes_used.get()};
        let mut peek_ptr = unsafe {
            (*current_frame_ptr.cast::<StackFrameHeader>()).current_frame_ptr
        };
        let mut curr_block_tail = unsafe {self.get_block_tail()};

        //because we're only searching within the scope,
        //we can assume the padding after the header
        //is key padding, because we shouldn't be expecting a header 
        //after the header we're looking in
        let stack_frame_ptr_after = {unsafe {
            let offset_ptr = current_frame_ptr.add(Self::SIZE_HEADER);
            let padding = offset_ptr.align_offset(Self::ALIGN_KEY);
            offset_ptr.add(padding)
        }};

        //we can't use the fun built-in library functions like align_offset,
        //so we do this math ourselves
        let value_padding = -(
            -(Self::SIZE_KEY as isize) % 
            Self::ALIGN_VALUE as isize
        ) as usize;
        let next_key_padding = -(
            -(Self::SIZE_VALUE as isize) % 
            Self::ALIGN_KEY as isize
        ) as usize;

        let key_value_size = Self::SIZE_KEY + value_padding +
            Self::SIZE_VALUE + next_key_padding;

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

                unsafe {
                    let offset = self.real_size().bytes() - bytes_remaining;
        
                    curr_block_tail = peek_ptr
                        .add(offset)
                        .cast::<BlockTail>()
                        .as_mut()
                        .expect("Error grabbing mutable reference to BlockTail");
                }
            }

            unsafe {
                peek_ptr = peek_ptr.sub(key_value_size);
                bytes_remaining -= key_value_size;
                let key_compare = (peek_ptr as *mut Key).as_ref_unchecked();
                let value = peek_ptr.add(Self::SIZE_KEY + next_key_padding)
                    .cast::<Value>();

                // eprintln!("comparing key {} with value {} at {:?} to key {}",
                //     key_compare, value.as_ref().unwrap(), peek_ptr, &key
                // );

                if key == *key_compare {
                    return Some(StackRef {
                        value,
                        phantom: PhantomData::default()
                    });
                }
            }
        }

        return None;
    }

    /// Finds the latest Value with that Key in the entire Stack, returning a StackRef to it.
    ///
    /// Allows you to dynamically grab values pushed into a frame
    /// by searching for its key.  Multiple Values can have the same key,
    /// so pushing a Value with a Key already used, 
    /// shadows the previous Value with that Key.  This function
    /// finds the last Value with that Key, so Values that are currently being shadowed,
    /// cannot be found by this function.  This function searches
    /// the entire Stack, so Values pushed at any time before this current Frame,
    /// as well as the values that have been pushed onto this Frame,
    /// can be found with this function.  If no Value contains this Key in the current Frame,
    /// this function will return a None.
    /// 
    /// # Examples
    /// 
    /// ```edition2020
    /// # use stack_frame_allocator::stack_frame_dict_allocator::StackFrameDictAllocator;
    /// 
    /// let stack = StackFrameDictAllocator::<&str, &str>::new();
    /// 
    /// stack.push("red", "old");
    /// stack.push("blue", "old");
    /// 
    /// stack.new_frame(|stack| {
    ///     stack.push("green", "new");
    /// 
    ///     let red = stack.get_in_stack("red").unwrap().get();
    ///     let blue = stack.get_in_stack("blue");
    ///     let green = stack.get_in_stack("green").unwrap().get();
    ///     assert_eq!(*red, "old");
    ///     assert!(blue.is_some());
    ///     assert_eq!(*green, "new");
    /// 
    ///     //shadow blue
    ///     stack.push("red", "new");
    /// 
    ///     let red = stack.get_in_stack("red").unwrap().get();
    ///     assert_eq!(*red, "new");
    /// });
    /// 
    /// let red = stack.get_in_stack("red").unwrap().get();
    /// let blue = stack.get_in_stack("blue").unwrap().get();
    /// assert_eq!(*red, "old");
    /// assert_eq!(*blue, "old");
    /// ```
    pub fn get_in_stack<'a>(
        &'a self, 
        key: impl Into<Key>
    ) -> Option<StackRef<'a, Value>> {
        let key = key.into();

        //we can't use the fun built-in library functions like align_offset,
        //so we do this math ourselves
        let value_padding = -(
            -(Self::SIZE_KEY as isize) % 
            Self::ALIGN_VALUE as isize
        ) as usize;
        let next_key_padding = -(
            -(Self::SIZE_VALUE as isize) % 
            Self::ALIGN_KEY as isize
        ) as usize;

        let key_value_size = Self::SIZE_KEY + value_padding +
            Self::SIZE_VALUE + next_key_padding;

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
            let padding = offset_ptr.align_offset(Self::ALIGN_KEY);
            offset_ptr.add(padding)
        }};

        //eprintln!("starting search at {:?} until {:?}", peek_ptr, stack_frame_ptr_after);
        loop {
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

                unsafe {
                    let offset = self.real_size().bytes() - bytes_remaining;
        
                    curr_block_tail = peek_ptr
                        .add(offset)
                        .cast::<BlockTail>()
                        .as_mut()
                        .expect("Error grabbing mutable reference to BlockTail");
                }
            }

            if peek_ptr < stack_frame_ptr_after {
                unreachable!("unexpected operation caused peek_ptr to go past the stack_frame_ptr");
            } else if peek_ptr == stack_frame_ptr_after {
                let Some(new_frame) = stack_frame.previous_frame else {
                    break;
                };

                stack_frame = new_frame;
                peek_ptr = stack_frame.current_frame_ptr;

                //this new header could have zero items
                just_jumped_block = false;
                expect_key_value_pair = false;
                stack_frame_ptr_after = unsafe {
                    let offset_ptr = (stack_frame as *const StackFrameHeader as *mut u8)
                        .add(Self::SIZE_HEADER);
                    let padding = offset_ptr.align_offset(Self::ALIGN_HEADER);
                    offset_ptr.add(padding)
                };

                continue;
            } else if !expect_key_value_pair || just_jumped_block {
                just_jumped_block = false;
                expect_key_value_pair = true;

                stack_frame_ptr_after = unsafe {
                    let offset_ptr = (stack_frame as *const StackFrameHeader as *mut u8)
                        .add(Self::SIZE_HEADER);
                    let padding = offset_ptr.align_offset(Self::ALIGN_KEY);
                    offset_ptr.add(padding)
                };
            }

            unsafe {
                peek_ptr = peek_ptr.sub(key_value_size);
                bytes_remaining -= key_value_size;
                let key_compare = (peek_ptr as *mut Key).as_ref_unchecked();
                let value = peek_ptr.add(Self::SIZE_KEY + next_key_padding)
                    .cast::<Value>();

                // eprintln!("comparing key {} with value {} at {:?} to key {}",
                //     key_compare, value.as_ref().unwrap(), peek_ptr, &key
                // );

                if key == *key_compare {
                    return Some(StackRef {
                        value,
                        phantom: PhantomData::default()
                    });
                }
            }
        }

        return None;
    }

    /// prints out the current stack from last push (top) to first push (bottom)
    /// 
    /// Includes where headers are.
    /// 
    /// # Examples
    /// 
    /// ```edition2020
    /// # use stack_frame_allocator::stack_frame_dict_allocator::StackFrameDictAllocator;
    /// 
    /// let stack = StackFrameDictAllocator::<&str, usize>::new();
    /// 
    /// stack.push("I", 0);
    /// stack.push("II", 1);
    /// stack.push("III", 2);
    /// 
    /// //first print
    /// stack.print();
    /// 
    /// stack.new_frame(|stack| {
    ///     stack.push("a", 3);
    ///     stack.push("b", 4);
    /// 
    ///     //second print
    ///     stack.print();
    /// 
    ///     unsafe { *stack.get_in_frame("b").unwrap().get_mut() = 69; }
    /// 
    ///     //third print
    ///     stack.print();
    /// });
    /// 
    /// stack.push("IV", 5);
    /// stack.push("V", 6);
    /// 
    /// //fourth print
    /// stack.print();
    /// 
    /// unsafe { *stack.get_in_frame("III").unwrap().get_mut() = 80085; }
    /// 
    /// //fifth print
    /// stack.print();
    /// ```
    /// 
    /// Will print out:
    /// ```text
    /// First print!
    /// 
    /// top of stack
    ///     Key: "III", Value: 2
    ///     Key: "II", Value: 1
    ///     Key: "I", Value: 0
    /// header
    /// 
    /// Second print!
    /// 
    /// top of stack
    ///     Key: "b", Value: 4
    ///     Key: "a", Value: 3
    /// header
    ///     Key: "III", Value: 2
    ///     Key: "II", Value: 1
    ///     Key: "I", Value: 0
    /// header
    /// 
    /// Third print!
    /// 
    /// top of stack
    ///     Key: "b", Value: 69
    ///     Key: "a", Value: 3
    /// header
    ///     Key: "III", Value: 2
    ///     Key: "II", Value: 1
    ///     Key: "I", Value: 0
    /// header
    /// 
    /// Fourth print!
    /// 
    /// top of stack
    ///     Key: "V", Value: 6
    ///     Key: "IV", Value: 5
    ///     Key: "III", Value: 2
    ///     Key: "II", Value: 1
    ///     Key: "I", Value: 0
    /// header
    /// 
    /// Fifth print!
    /// 
    /// top of stack
    ///     Key: "V", Value: 6
    ///     Key: "IV", Value: 5
    ///     Key: "III", Value: 80085
    ///     Key: "II", Value: 1
    ///     Key: "I", Value: 0
    /// header
    /// ```
    pub fn print(&self) where Key: Display, Value: Display {
        let mut count_blocks = 1;

        //we can't use the fun built-in library functions like align_offset,
        //so we do this math ourselves
        let value_padding = -(
            -(Self::SIZE_KEY as isize) % 
            Self::ALIGN_VALUE as isize
        ) as usize;
        let next_key_padding = -(
            -(Self::SIZE_VALUE as isize) % 
            Self::ALIGN_KEY as isize
        ) as usize;

        let key_value_size = Self::SIZE_KEY + value_padding +
            Self::SIZE_VALUE + next_key_padding;

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
            let padding = offset_ptr.align_offset(Self::ALIGN_KEY);
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
                    let padding = offset_ptr.align_offset(Self::ALIGN_KEY);
                    offset_ptr.add(padding)
                };
            }
            
            peek_ptr = peek_ptr.sub(key_value_size);
            let key = peek_ptr.cast::<Key>().as_ref().unwrap_unchecked();
            let value = peek_ptr.add(Self::SIZE_KEY + value_padding)
                .cast::<Value>().as_ref().unwrap_unchecked();
            println!("\tKey: {}, Value: {}", key, value);
        }}

        println!("\n{} block(s) of size {} bytes have been allocated.\n", 
            count_blocks, 
            self.size.bytes()
        );
    }

    //TODO add allocated_blocks(&self) -> usize and using_blocks(&self) -> usize functions
}

impl<'s, Key, Value> Drop for StackFrameDictAllocator<'s, Key, Value> 
where 
    Key: Eq + Hash
{
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
                let padding = offset_ptr.align_offset(Self::ALIGN_KEY);
                offset_ptr.add(padding)
            };
    
            //we can't use the fun built-in library functions like align_offset,
            //so we do this math ourselves
            let value_padding = -(
                -(Self::SIZE_KEY as isize) % 
                Self::ALIGN_VALUE as isize
            ) as usize;
            let next_key_padding = -(
                -(Self::SIZE_VALUE as isize) % 
                Self::ALIGN_KEY as isize
            ) as usize;
    
            let key_value_size = Self::SIZE_KEY + value_padding +
                Self::SIZE_VALUE + next_key_padding;
    
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
                peek_ptr = peek_ptr.sub(key_value_size);
                bytes_remaining -= key_value_size;
                
                std::ptr::drop_in_place(peek_ptr as *mut Key);
                std::ptr::drop_in_place(peek_ptr.add(Self::SIZE_KEY + next_key_padding)
                    .cast::<Value>()
                );
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

    #[test]
    pub fn get_in_frame_test() {
        let stack = StackFrameDictAllocator::<&str, &str>::new();
        
        stack.push("red", "first");
        stack.push("blue", "first");
        
        stack.new_scope(|stack| {
            stack.push("red", "second");
        
            let red = stack.get_in_frame("red").unwrap().get();
            let blue = stack.get_in_frame("blue");
            assert_eq!(*red, "second");
            assert!(blue.is_none());
        });
        
        let red = stack.get_in_frame("red").unwrap().get();
        let blue = stack.get_in_frame("blue").unwrap().get();
        assert_eq!(*red, "first");
        assert_eq!(*blue, "first");
        
        //shadow blue
        stack.push("blue", "second");
        
        let blue = stack.get_in_frame("blue").unwrap().get();
        assert_eq!(*blue, "second");
    }

    #[test]
    pub fn get_in_stack_test() {
        let stack = StackFrameDictAllocator::<&str, &str>::new();
    
        stack.push("red", "old");
        stack.push("blue", "old");
    
        stack.new_scope(|stack| {
            stack.push("green", "new");
    
            let red = stack.get_in_stack("red").unwrap().get();
            let blue = stack.get_in_stack("blue");
            let green = stack.get_in_stack("green").unwrap().get();
            assert_eq!(*red, "old");
            assert!(blue.is_some());
            assert_eq!(*green, "new");
    
            //shadow blue
            stack.push("red", "new");
    
            let red = stack.get_in_stack("red").unwrap().get();
            assert_eq!(*red, "new");
        });

        let red = stack.get_in_stack("red").unwrap().get();
        let blue = stack.get_in_stack("blue").unwrap().get();
        assert_eq!(*red, "old");
        assert_eq!(*blue, "old");
    }

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
    pub fn drop_test() {
        let dropped = RefCell::new(vec![]);
        {
            let stack = StackFrameDictAllocator::<DropPrint<&str>, DropTest>::new();
            stack.push(DropPrint("key1scope1"), DropTest("value1scope1", &dropped));
            stack.push(DropPrint("key2scope1"), DropTest("value2scope1", &dropped));
            stack.push(DropPrint("key3scope1"), DropTest("value3scope1", &dropped));
            stack.new_scope(|stack| {
                stack.push(DropPrint("key1scope2"), DropTest("value1scope2", &dropped));
                stack.push(DropPrint("key2scope2"), DropTest("value2scope2", &dropped));
                stack.push(DropPrint("key3scope2"), DropTest("value3scope2", &dropped));
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
    #[allow(unused_variables)]
    pub fn empty_drop_test() {
        let stack_u8_u8 = StackFrameDictAllocator::<DropPrint<u8>, DropPrint<u8>>::new();
        let stack_u8_u16 = StackFrameDictAllocator::<DropPrint<u8>, DropPrint<u16>>::new();
        let stack_u8_u32 = StackFrameDictAllocator::<DropPrint<u8>, DropPrint<u32>>::new();
        let stack_u8_u64 = StackFrameDictAllocator::<DropPrint<u8>, DropPrint<u64>>::new();
        let stack_u8_u128 = StackFrameDictAllocator::<DropPrint<u8>, DropPrint<u128>>::new();

        let stack_u16_u8 = StackFrameDictAllocator::<DropPrint<u16>, DropPrint<u8>>::new();
        let stack_u16_u16 = StackFrameDictAllocator::<DropPrint<u16>, DropPrint<u16>>::new();
        let stack_u16_u32 = StackFrameDictAllocator::<DropPrint<u16>, DropPrint<u32>>::new();
        let stack_u16_u64 = StackFrameDictAllocator::<DropPrint<u16>, DropPrint<u64>>::new();
        let stack_u16_u128 = StackFrameDictAllocator::<DropPrint<u16>, DropPrint<u128>>::new();

        let stack_u32_u8 = StackFrameDictAllocator::<DropPrint<u32>, DropPrint<u8>>::new();
        let stack_u32_u16 = StackFrameDictAllocator::<DropPrint<u32>, DropPrint<u16>>::new();
        let stack_u32_u32 = StackFrameDictAllocator::<DropPrint<u32>, DropPrint<u32>>::new();
        let stack_u32_u64 = StackFrameDictAllocator::<DropPrint<u32>, DropPrint<u64>>::new();
        let stack_u32_u128 = StackFrameDictAllocator::<DropPrint<u32>, DropPrint<u128>>::new();

        let stack_u64_u8 = StackFrameDictAllocator::<DropPrint<u64>, DropPrint<u8>>::new();
        let stack_u64_u16 = StackFrameDictAllocator::<DropPrint<u64>, DropPrint<u16>>::new();
        let stack_u64_u32 = StackFrameDictAllocator::<DropPrint<u64>, DropPrint<u32>>::new();
        let stack_u64_u64 = StackFrameDictAllocator::<DropPrint<u64>, DropPrint<u64>>::new();
        let stack_u64_u128 = StackFrameDictAllocator::<DropPrint<u64>, DropPrint<u128>>::new();

        let stack_u128_u8 = StackFrameDictAllocator::<DropPrint<u128>, DropPrint<u8>>::new();
        let stack_u128_u16 = StackFrameDictAllocator::<DropPrint<u128>, DropPrint<u16>>::new();
        let stack_u128_u32 = StackFrameDictAllocator::<DropPrint<u128>, DropPrint<u32>>::new();
        let stack_u128_u64 = StackFrameDictAllocator::<DropPrint<u128>, DropPrint<u64>>::new();
        let stack_u128_u128 = StackFrameDictAllocator::<DropPrint<u128>, DropPrint<u128>>::new();
    }
}