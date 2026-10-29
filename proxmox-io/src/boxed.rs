/// Zero-initialized bytes, meant for large sizes to avoid busting the stack.
pub fn zeroed(len: usize) -> Box<[u8]> {
    // SAFETY: zero is a valid bit-pattern for u8
    unsafe { Box::new_zeroed_slice(len).assume_init() }
}
