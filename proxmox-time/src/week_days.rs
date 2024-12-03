use bitflags::bitflags;
use nom::{bytes::complete::tag, character::complete::alpha1, combinator::opt, sequence::pair};

use crate::parse_helpers::{parse_error, IResult};

bitflags! {
    /// Defines one or more days of a week.
    #[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
    pub struct WeekDays: u8 {
        const MONDAY = 1;
        const TUESDAY = 2;
        const WEDNESDAY = 4;
        const THURSDAY = 8;
        const FRIDAY = 16;
        const SATURDAY = 32;
        const SUNDAY = 64;
    }
}

fn parse_weekday(i: &str) -> IResult<&str, WeekDays> {
    let (i, text) = alpha1(i)?;

    match text.to_ascii_lowercase().as_str() {
        "monday" | "mon" => Ok((i, WeekDays::MONDAY)),
        "tuesday" | "tue" => Ok((i, WeekDays::TUESDAY)),
        "wednesday" | "wed" => Ok((i, WeekDays::WEDNESDAY)),
        "thursday" | "thu" => Ok((i, WeekDays::THURSDAY)),
        "friday" | "fri" => Ok((i, WeekDays::FRIDAY)),
        "saturday" | "sat" => Ok((i, WeekDays::SATURDAY)),
        "sunday" | "sun" => Ok((i, WeekDays::SUNDAY)),
        _ => Err(parse_error(text, "weekday")),
    }
}

pub(crate) fn parse_weekdays_range(i: &str) -> IResult<&str, WeekDays> {
    let (i, startday) = parse_weekday(i)?;

    let generate_range = |start, end| {
        let mut res = 0;
        let mut pos = start;
        loop {
            res |= pos;
            if pos >= end {
                break;
            }
            pos <<= 1;
        }
        WeekDays::from_bits(res).unwrap()
    };

    if let (i, Some((_, endday))) = opt(pair(tag(".."), parse_weekday))(i)? {
        let start = startday.bits();
        let end = endday.bits();
        if start > end {
            let set1 = generate_range(start, WeekDays::SUNDAY.bits());
            let set2 = generate_range(WeekDays::MONDAY.bits(), end);
            Ok((i, set1 | set2))
        } else {
            Ok((i, generate_range(start, end)))
        }
    } else {
        Ok((i, startday))
    }
}
