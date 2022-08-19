use anyhow::{bail, Error};

use super::daily_duration::*;

use nom::{
    bytes::complete::tag,
    character::complete::digit1,
    combinator::{all_consuming, map_res, opt, recognize},
    error::{ContextError, VerboseError},
    sequence::{preceded, tuple},
};

pub(crate) type IResult<I, O, E = VerboseError<I>> = Result<(I, O), nom::Err<E>>;

pub(crate) fn parse_error<'a>(
    i: &'a str,
    context: &'static str,
) -> nom::Err<VerboseError<&'a str>> {
    let err = VerboseError { errors: Vec::new() };
    let err = VerboseError::add_context(i, context, err);
    nom::Err::Error(err)
}

// Parse a 64 bit unsigned integer
pub(crate) fn parse_u64(i: &str) -> IResult<&str, u64> {
    map_res(recognize(digit1), str::parse)(i)
}

// Parse complete input, generate simple error message (use this for sinple line input).
pub(crate) fn parse_complete_line<'a, F, O>(what: &str, i: &'a str, parser: F) -> Result<O, Error>
where
    F: Fn(&'a str) -> IResult<&'a str, O>,
{
    match all_consuming(parser)(i) {
        Err(nom::Err::Error(VerboseError { errors }))
        | Err(nom::Err::Failure(VerboseError { errors })) => {
            if errors.is_empty() {
                bail!("unable to parse {}", what);
            } else {
                bail!(
                    "unable to parse {} at '{}' - {:?}",
                    what,
                    errors[0].0,
                    errors[0].1
                );
            }
        }
        Err(err) => {
            bail!("unable to parse {} - {}", what, err);
        }
        Ok((_, data)) => Ok(data),
    }
}

pub(crate) fn parse_time_comp(max: usize) -> impl Fn(&str) -> IResult<&str, u32> {
    move |i: &str| {
        let (i, v) = map_res(recognize(digit1), str::parse)(i)?;
        if (v as usize) >= max {
            return Err(parse_error(i, "time value too large"));
        }
        Ok((i, v))
    }
}

pub(crate) fn parse_hm_time(i: &str) -> IResult<&str, HmTime> {
    let (i, (hour, opt_minute)) = tuple((
        parse_time_comp(24),
        opt(preceded(tag(":"), parse_time_comp(60))),
    ))(i)?;

    match opt_minute {
        Some(minute) => Ok((i, HmTime { hour, minute })),
        None => Ok((i, HmTime { hour, minute: 0 })),
    }
}
