use std::error::Error as StdError;
use std::fmt;

/// We do not want the `From<base64::*>` impls to be publicly available, so we use this wrapper
/// instead.
pub(crate) trait ConvertError {
    type ConvertError;

    fn convert_error(self) -> Self::ConvertError;
}

impl<T, E> ConvertError for Result<T, E>
where
    E: ConvertError,
{
    type ConvertError = Result<T, E::ConvertError>;
    fn convert_error(self) -> Self::ConvertError {
        self.map_err(E::convert_error)
    }
}

/// A base64 encoding error.
///
/// Currently the only error when encoding can be when encoding directly into a preallocated slice
/// which is too small.
#[derive(Debug)]
#[non_exhaustive]
pub enum EncodeError {
    /// The output was too small.
    NoSpace,
}

impl ConvertError for base64::EncodeSliceError {
    type ConvertError = EncodeError;

    fn convert_error(self) -> Self::ConvertError {
        match self {
            Self::OutputSliceTooSmall => EncodeError::NoSpace,
        }
    }
}

impl EncodeError {
    /// True if this error happened because of insufficient space in the output slice.
    pub fn is_no_space(&self) -> bool {
        matches!(self, Self::NoSpace)
    }
}

impl fmt::Display for EncodeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::NoSpace => f.write_str("insufficient space in output slice"),
        }
    }
}

impl StdError for EncodeError {}

/// A base64 decoding error.
///
/// Apart from the case where the output slice is too small, the exact error is not directly
/// available as an enum variant.
pub struct DecodeError(DecodeErrorImpl);

impl fmt::Debug for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&self.0, f)
    }
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl StdError for DecodeError {
    /// While we do provide the `base64` crate's error as a source *if possible*, it is NOT
    /// API-stable and therefore not recommended to be used.
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.0.source()
    }
}

impl DecodeError {
    /// True if this error happened because of insufficient space in the outputslice.
    pub fn is_no_space(&self) -> bool {
        matches!(&self.0, DecodeErrorImpl::NoSpace(_))
    }

    /// Unexpected padding or lack thereof.
    ///
    /// Either the input lacked padding in a decoding method which requires it, or it was padded
    /// while using a decoding method which rejects it.
    pub fn is_invlaid_padding(&self) -> bool {
        matches!(&self.0, DecodeErrorImpl::InvalidPadding(_))
    }
}

#[derive(Debug)]
enum DecodeErrorImpl {
    /// The output was too small.
    NoSpace(base64::DecodeSliceError),

    /// The input had invalid padding.
    InvalidPadding(base64::DecodeError),

    /// Other unspecified errors.
    Other(base64::DecodeError),
}

impl ConvertError for base64::DecodeError {
    type ConvertError = DecodeError;

    fn convert_error(self) -> Self::ConvertError {
        DecodeError(match self {
            err @ Self::InvalidPadding => DecodeErrorImpl::InvalidPadding(err),
            err => DecodeErrorImpl::Other(err),
        })
    }
}

impl ConvertError for base64::DecodeSliceError {
    type ConvertError = DecodeError;

    fn convert_error(self) -> Self::ConvertError {
        DecodeError(match self {
            err @ Self::OutputSliceTooSmall => DecodeErrorImpl::NoSpace(err),
            Self::DecodeError(err) => return err.convert_error(),
        })
    }
}

impl fmt::Display for DecodeErrorImpl {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::NoSpace(_) => f.write_str("insufficient space in output slice"),
            Self::InvalidPadding(_) => f.write_str("invalid padding in base64 encoded data"),
            Self::Other(_) => f.write_str("error decoding base64 data"),
        }
    }
}

impl StdError for DecodeErrorImpl {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::NoSpace(e) => Some(e),
            Self::InvalidPadding(e) => Some(e),
            Self::Other(e) => Some(e),
        }
    }
}
