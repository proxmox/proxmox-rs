use anyhow::{bail, Error};

use nom::{
    bytes::complete::take_while1,
    combinator::all_consuming,
    error::{ContextError, VerboseError},
};

pub(crate) type IResult<I, O, E = VerboseError<I>> = Result<(I, O), nom::Err<E>>;

fn verbose_err<'a>(i: &'a str, ctx: &'static str) -> VerboseError<&'a str> {
    VerboseError::add_context(i, ctx, VerboseError { errors: vec![] })
}

pub(crate) fn parse_error<'a>(
    i: &'a str,
    context: &'static str,
) -> nom::Err<VerboseError<&'a str>> {
    nom::Err::Error(verbose_err(i, context))
}

pub(crate) fn parse_failure<'a>(
    i: &'a str,
    context: &'static str,
) -> nom::Err<VerboseError<&'a str>> {
    nom::Err::Error(verbose_err(i, context))
}

/// Recognizes one or more non-whitespace characters
pub(crate) fn notspace1(i: &str) -> IResult<&str, &str> {
    take_while1(|c| !(c == ' ' || c == '\t' || c == '\n'))(i)
}

/// Parse complete input, generate verbose error message with line numbers
pub(crate) fn parse_complete<'a, F, O>(what: &str, i: &'a str, parser: F) -> Result<O, Error>
where
    F: FnMut(&'a str) -> IResult<&'a str, O>,
{
    match all_consuming(parser)(i) {
        Err(nom::Err::Error(err)) | Err(nom::Err::Failure(err)) => {
            bail!(
                "unable to parse {} - {}",
                what,
                nom::error::convert_error(i, err)
            );
        }
        Err(err) => {
            bail!("unable to parse {} - {}", what, err);
        }
        Ok((_, data)) => Ok(data),
    }
}
