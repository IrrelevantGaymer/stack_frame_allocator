# Stack Frame Allocators

Stack Frame Allocators are based off the concept of the Stack and creating "Frames" where values can be pushed onto this frame.  Instead of popping individual items,
you pop an entire Frame including its items.  Frames are directly analogous to scopes, so in order to create a new Frame, you must create a new scope for it.
Frames only exist in the scope they're created in, so the lifetime of a pushed value and any references to this value is directly linked to the lifetime of its frame/scope,
At the end of a frame's scope, the entire frame is popped, and the StackFrameAllocator will continue pushing items onto the previous frame.  
It's like putting a board on a table, and putting plates on that board; you can place a new board on top of those plates, and then place more plates on top, ad infinitum.
Instead of removing individual plates, you remove a board including all the plates on top of it, and you can only remove a board if there's no other boards on top of it.

For the purposes of brevity, I may sometimes refer to any of the Stack Frame Allocators as "the Stack".

## The Allocators

### Stack Frame Allocator

The Stack Frame Allocator is the most simple of the Stack Frame Allocators.  The Stack can accept values of a generic type Value.  When pushing a value, 
you get a reference wrapper to the value.  The lifetime of the reference is the lifetime of the frame.

### Stack Frame Dict Allocator

The Stack Frame Dict Allocator allows you to push key value pairs of generic types Key and Value onto the Stack.  When pushing a key value pair,
you get a reference wrapper to the value; however, you can also obtain a reference wrapper to the value by searching for its key.  Because of this,
grabbing a mutable reference from the wrapper is considered unsafe, due to the fact that multiple reference wrappers to the same value can exist at the same time,
and it is impossible for the borrow checker to determine at compile time if the borrowing rules are broken in this instance.  So instead of grabbing a mutable reference,
it is recommended that the generic type Value should be wrapped in an interior mutable struct such as Cell, RefCell, RwLock, etc. so that you can have multiple
immutable references and be able to safely mutate the value.

### Other Potential Allocators

A General Stack Frame Allocator could be made such that data of any type can be pushed onto the same Stack, however one QoL method the Stack Frame and Stack Frame Dict Allocators
implement is print: a General Stack Frame Allocator would either not be able to implement print, or require that extra data be pushed such that we can walk through the Stack
to print it out.  For my general purposes, I only need the Stack Frame and Stack Frame Dict Allocators, though I may in the future implement a General Stack Frame Allocator.
