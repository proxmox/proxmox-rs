/// Note: this only compares *bytes* and is not strictly speaking equivalent to str::cmp!
pub const fn byte_string_cmp(a: &[u8], b: &[u8]) -> std::cmp::Ordering {
    use std::cmp::Ordering::*;

    // const-version of `min(a.len(), b.len())` while simultaneously remembering
    // `cmp(a.len(), b.len())`.
    let (end, len_result) = if a.len() < b.len() {
        (a.len(), Less)
    } else if a.len() > b.len() {
        (b.len(), Greater)
    } else {
        (a.len(), Equal)
    };

    let mut i = 0;
    while i != end {
        if a[i] < b[i] {
            return Less;
        } else if a[i] > b[i] {
            return Greater;
        }
        i += 1;
    }
    len_result
}
