use std::{error::Error, fmt::Display};

use crate::platform::PlatformError;

#[derive(Debug)]
pub enum BusmanError {
	Decode(DecodeError),
	Encode(EncodeError),
	Parse(ParseError),
	Platform(PlatformError),
	Io(std::io::Error),
}

impl Error for BusmanError {}

impl From<std::io::Error> for BusmanError {
	fn from(value: std::io::Error) -> Self {
		Self::Io(value)
	}
}

impl From<std::str::Utf8Error> for BusmanError {
	fn from(value: std::str::Utf8Error) -> Self {
		Self::Decode(DecodeError::InvalidUtf8(value))
	}
}

impl From<EncodeError> for BusmanError {
	fn from(value: EncodeError) -> Self {
		Self::Encode(value)
	}
}

impl From<DecodeError> for BusmanError {
	fn from(value: DecodeError) -> Self {
		Self::Decode(value)
	}
}

impl From<ParseError> for BusmanError {
	fn from(value: ParseError) -> Self {
		Self::Parse(value)
	}
}

impl From<PlatformError> for BusmanError {
	fn from(value: PlatformError) -> Self {
		Self::Platform(value)
	}
}

impl Display for BusmanError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			BusmanError::Decode(e) => e.fmt(f),
			BusmanError::Encode(e) => e.fmt(f),
			BusmanError::Parse(e) => e.fmt(f),
			BusmanError::Platform(e) => e.fmt(f),
			BusmanError::Io(e) => e.fmt(f),
		}
	}
}

#[derive(Debug)]
pub enum EncodeError {
	StringExceedsFieldWidth(String, usize),
	Io(std::io::Error),
}

impl Error for EncodeError {}

impl From<std::io::Error> for EncodeError {
	fn from(value: std::io::Error) -> Self {
		Self::Io(value)
	}
}

impl Display for EncodeError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			EncodeError::StringExceedsFieldWidth(s, size) => {
				write!(f, "string of length {} exceeds field width {size}", s.len())
			}
			EncodeError::Io(_) => write!(f, "{self:?}"),
		}
	}
}

#[derive(Debug)]
pub enum DecodeError {
	UnsupportedOpcode(u16),
	Io(std::io::Error),
	InvalidUtf8(std::str::Utf8Error),
}

impl Error for DecodeError {}

impl From<std::io::Error> for DecodeError {
	fn from(value: std::io::Error) -> Self {
		Self::Io(value)
	}
}

impl From<std::str::Utf8Error> for DecodeError {
	fn from(value: std::str::Utf8Error) -> Self {
		Self::InvalidUtf8(value)
	}
}

impl Display for DecodeError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::UnsupportedOpcode(e) => write!(f, "unsupported opcode: {:#06x}", e),
			_ => write!(f, "{self:?}"),
		}
	}
}

#[derive(Debug)]
pub enum ParseError {
	NotADirectory,
	NotAnInterface,
	NotADevice,
	NonUtf8Path,
	Io(std::io::Error),
	InvalidHex(std::num::ParseIntError),
}

impl From<std::io::Error> for ParseError {
	fn from(value: std::io::Error) -> Self {
		Self::Io(value)
	}
}

impl From<std::num::ParseIntError> for ParseError {
	fn from(value: std::num::ParseIntError) -> Self {
		Self::InvalidHex(value)
	}
}

impl Display for ParseError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{self:?}")
	}
}
