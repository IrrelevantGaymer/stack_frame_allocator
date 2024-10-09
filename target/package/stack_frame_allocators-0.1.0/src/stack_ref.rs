//! General Wrapper for References within either of the crate's Stack Allocators:
//! the StackFrameDictAllocator, and the StackFrameAllocator.
//! Grabbing values from a StackFrameDictAllocator gives you unsafe_ref::StackRef's
//! because mutltiple StackRefs can be obtained which all point to the same value,
//! thus you could make multiple mutable references to the same value which
//! violates the rules of the borrow checker.
//! Grabbing values from a StackFrameAllocator gives you safe_ref::StackRef's
//! because only one StackRef can point to a value at any given time, which
//! means the borrow checker can validate that borrowing rules are being followed.
//! There is also a static guarantee that the lifetime of a StackRef is the same
//! lifetime of the Frame of the Value the StackRef is pointing to.

/// Logic for StackRef where grabbing a mutable reference can potentially be unsafe,
/// because it is impossible for the borrow checker to validate borrowing rules at compile time.
/// StackRefs grabbed from a [StackFrameDictAllocator] will be [unsafe_ref::StackRef]
pub mod unsafe_ref {

    //TODO Consider get_in_frame and get_in_stack methods taking a reference to a key, 
    //TODO to help enforce borrow checker rules. One issue is people can just make new copies of a key.
    //TODO another approach is keys are always stored as references within the allocator. and their addresses
    //TODO are compared instead of their contents, however this would make using the Allocator
    //TODO less ergonomic 

    use std::marker::PhantomData;

    /// Returned by StackFrameAllocator, StackFrameGeneralAllocator, and StackFrameDictAllocator
    /// 
    /// A wrapper for references to data within one of these allocators.  Ensures compile-time
    /// safety for the lifetime of these references.  StackRefs can only live as long as the current
    /// StackFrame regardless if the StackRef points to a piece of data within that frame.
    /// 
    /// # Safety
    /// 
    /// Whenever calling [get_mut](crate::stack_ref::StackRef::get_mut), the caller must ensure
    /// that the borrow checker rules are followed.  The user can avoid 
    /// [get_mut](crate::stack_ref::StackRef::get_mut) by only using
    /// Allocators where values are wrapped in a type with interior mutability

    pub struct StackRef<'a, T> {
        pub(crate) value: *mut T,
        pub(crate) phantom: PhantomData<&'a T>
    }

    impl<'a, T> StackRef<'a, T> {
        /// Grabs an immutable reference to the value StackRef points to
        /// 
        /// StackRef's will guarantee that any reference created by a StackRef
        /// is valid until the next StackFrame is popped[^note].
        /// See also [get_mut](crate::stack_ref::StackRef::get_mut).
        /// 
        /// [^note]: Unless you use unsafe function get_mut which requires 
        /// the user to validate borrowing rules themselves.
        /// 
        /// # Examples
        /// 
        /// ```rust
        /// # use stack_frame_allocator::stack_frame_dict_allocator::StackFrameDictAllocator;
        /// 
        /// let stack = StackFrameDictAllocator::<&str, usize>::new();
        /// stack.push("a", 80085);
        /// stack.push("b", 420);
        /// stack.push("c", 69);
        /// 
        /// let a = stack.get_in_frame("a").unwrap().get();
        /// let b = stack.get_in_frame("b").unwrap().get();
        /// let c = stack.get_in_frame("c").unwrap().get();
        /// 
        /// assert_eq!(*a, 80085);
        /// assert_eq!(*b, 420);
        /// assert_eq!(*c, 69);
        /// ```
        pub fn get(&self) -> &'a T {
            unsafe {self.value.as_ref_unchecked()}
        }

        /// Because StackRefs can be dynamically obtained
        /// the borrow checker can't always determine if
        /// borrowing rules are violated.  Only use this
        /// function if you yourself can verify that borrowing
        /// rules are followed.  If you want to safely mutate
        /// the stack's data, wrap the Value type in
        /// a Interior Mutable structure like RefCell.
        /// See also [get](crate::stack_ref::StackRef::get).
        /// 
        /// # Examples
        /// 
        /// ```edition2020
        /// # use stack_frame_allocator::stack_frame_dict_allocator::StackFrameDictAllocator;
        /// 
        /// let stack = StackFrameDictAllocator::<&str, usize>::new();
        /// stack.push("a", 0);
        /// 
        /// let a = unsafe {
        ///     stack.get_in_frame("a").unwrap().get_mut()
        /// };
        /// //This violates the rules of the borrow checker
        /// //But there's no error
        /// let bad_a = stack.get_in_frame("a").unwrap().get();
        /// 
        /// *a = 1;
        /// ```
        /// ```edition2020
        /// /* Better Alternative */
        /// # use stack_frame_allocator::stack_frame_dict_allocator::StackFrameDictAllocator;
        /// 
        /// use std::cell::RefCell;
        /// 
        /// let stack = StackFrameDictAllocator::<&str, RefCell<usize>>::new();
        /// stack.push("a", RefCell::new(0));
        /// 
        /// let mut a = stack.get_in_frame("a").unwrap().get().borrow_mut();
        /// 
        /// //uncommenting the next line will error at runtime due to RefCell's guarantee
        /// //of maintaining the rules of the borrow checker at runtime
        /// 
        /// //let bad_a = stack.get_in_frame("a").unwrap().get().borrow();
        /// 
        /// *a = 1;
        /// ```
        pub unsafe fn get_mut(&mut self) -> &'a mut T {
            unsafe {self.value.as_mut_unchecked()}
        }
    }
}

/// Logic for StackRef where grabbing a mutable reference is safe, 
/// and borrowing rules are validated at compile time by the borrow checker.
/// StackRefs grabbed from a [StackFrameAllocator] will be [safe_ref::StackRef]
pub mod safe_ref {
    use std::marker::PhantomData;

    /// Returned by StackFrameAllocator, StackFrameGeneralAllocator, and StackFrameDictAllocator
    /// 
    /// A wrapper for references to data within one of these allocators.  Ensures compile-time
    /// safety for the lifetime of these references.  StackRefs can only live as long as the current
    /// StackFrame regardless if the StackRef points to a piece of data within that frame.
    /// 
    /// # Safety
    /// 
    /// Whenever calling [get_mut](crate::stack_ref::StackRef::get_mut), the caller must ensure
    /// that the borrow checker rules are followed.  The user can avoid 
    /// [get_mut](crate::stack_ref::StackRef::get_mut) by only using
    /// Allocators where values are wrapped in a type with interior mutability

    pub struct StackRef<'a, T> {
        pub(crate) value: *mut T,
        pub(crate) phantom: PhantomData<&'a T>
    }

    impl<'a, T> StackRef<'a, T> {
        /// Grabs an immutable reference to the value StackRef points to
        /// 
        /// StackRef's will guarantee that any reference created by a StackRef
        /// is valid until the next StackFrame is popped.
        /// See also [get_mut](crate::stack_ref::StackRef::get_mut).
        /// 
        /// # Examples
        /// 
        /// ```rust
        /// # use stack_frame_allocator::stack_frame_allocator::StackFrameAllocator;
        /// 
        /// let stack = StackFrameAllocator::<usize>::new();
        /// let a = stack.push(80085).get();
        /// let b = stack.push(420).get();
        /// let c = stack.push(69).get();
        /// 
        /// assert_eq!(*a, 80085);
        /// assert_eq!(*b, 420);
        /// assert_eq!(*c, 69);
        /// ```
        pub fn get(&self) -> &'a T {
            unsafe {self.value.as_ref_unchecked()}
        }

        /// Grabs a mutable reference to the value StackRef points to.
        /// 
        /// For the StackFrameAllocator, only one StackRef for a given value 
        /// can exist at any given moment, so this is a safe operation,
        /// because the borrow checker at compile time can verify that
        /// there's only one mutable reference to the value.  The reference
        /// is also guaranteed to be valid until the frame the value is in
        /// drops.
        /// 
        /// # Examples
        /// 
        /// ```edition2020
        /// # use stack_frame_allocator::stack_frame_allocator::StackFrameAllocator;
        /// 
        /// let stack = StackFrameAllocator::<usize>::new();
        /// let mut a = stack.push(1).get_mut();
        /// let mut b = stack.push(2).get_mut();
        /// let mut c = stack.push(3).get_mut();
        /// 
        /// assert_eq!(*a, 1);
        /// assert_eq!(*b, 2);
        /// assert_eq!(*c, 3);
        /// 
        /// *a = 80085;
        /// *b = 420;
        /// *c = 69;
        /// 
        /// assert_eq!(*a, 80085);
        /// assert_eq!(*b, 420);
        /// assert_eq!(*c, 69);
        /// ```
        pub fn get_mut(&mut self) -> &'a mut T {
            unsafe {self.value.as_mut_unchecked()}
        }
    }
}