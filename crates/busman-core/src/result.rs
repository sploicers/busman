use crate::error::{BusmanError, DecodeError, EncodeError};

pub type Result<T> = core::result::Result<T, BusmanError>;

pub type EncodeResult<T> = core::result::Result<T, EncodeError>;
pub type DecodeResult<T> = core::result::Result<T, DecodeError>;
