/// Uninitialized bytes, without `MaybeUninit`.
///
/// We're talking about bytes here, which we *allocate*. That is, we call a function, get a pointer
/// to stuff, and can use it. It's not UB in that there's nothing undefined about what's going on
/// here.
pub fn uninitialized(len: usize) -> Box<[u8]> {
    unsafe {
        let data = std::alloc::alloc(std::alloc::Layout::array::<u8>(len).unwrap());
        Box::from_raw(std::ptr::slice_from_raw_parts_mut(data, len))
    }
}

/// Zero-initialized bytes, meant for large sizes to avoid busting the stack.
pub fn zeroed(len: usize) -> Box<[u8]> {
    // SAFETY: zero is a valid bit-pattern for u8
    unsafe { Box::new_zeroed_slice(len).assume_init() }
}
