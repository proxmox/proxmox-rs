#[derive(Debug, Clone)]
pub(crate) enum DateTimeValue {
    Single(u32),
    Range(u32, u32),
    Repeated(u32, u32, Option<u32>),
}

#[cfg(not(target_arch = "wasm32"))]
impl DateTimeValue {
    // Test if the entry contains the value
    pub fn contains(&self, value: u32) -> bool {
        match self {
            DateTimeValue::Single(v) => *v == value,
            DateTimeValue::Range(start, end) => value >= *start && value <= *end,
            DateTimeValue::Repeated(start, repetition, opt_end) => {
                if value >= *start {
                    if *repetition > 0 {
                        let offset = value - start;
                        let res = offset.is_multiple_of(*repetition);
                        if let Some(end) = opt_end {
                            res && value <= *end
                        } else {
                            res
                        }
                    } else {
                        *start == value
                    }
                } else {
                    false
                }
            }
        }
    }

    pub fn list_contains(list: &[DateTimeValue], value: u32) -> bool {
        list.iter().any(|spec| spec.contains(value))
    }

    // Find an return an entry greater than value
    pub fn find_next(list: &[DateTimeValue], value: u32) -> Option<u32> {
        let mut next: Option<u32> = None;
        let mut set_next = |v: u32| {
            if let Some(n) = next {
                if v < n {
                    next = Some(v);
                }
            } else {
                next = Some(v);
            }
        };
        for spec in list {
            match spec {
                DateTimeValue::Single(v) => {
                    if *v > value {
                        set_next(*v);
                    }
                }
                DateTimeValue::Range(start, end) => {
                    if value < *start {
                        set_next(*start);
                    } else {
                        let n = value + 1;
                        if n >= *start && n <= *end {
                            set_next(n);
                        }
                    }
                }
                DateTimeValue::Repeated(start, repetition, opt_end) => {
                    if value < *start {
                        set_next(*start);
                    } else if *repetition > 0 {
                        let n = start + ((value - start + repetition) / repetition) * repetition;
                        if let Some(end) = opt_end {
                            if n <= *end {
                                set_next(n);
                            }
                        } else {
                            set_next(n);
                        }
                    }
                }
            }
        }

        next
    }
}
