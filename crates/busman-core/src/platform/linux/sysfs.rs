use num_traits::Num;
use std::path::Path;

use crate::error::ParseError;

pub fn read_sysfs_val_string(path: &Path) -> std::result::Result<String, ParseError> {
	let s = std::fs::read_to_string(path)?;
	Ok(s.trim().to_owned())
}

pub fn write_sysfs_val_string<T: Into<String>>(path: &Path, val: T) -> std::result::Result<(), ParseError> {
	Ok(std::fs::write(path, val.into())?)
}

pub fn read_sysfs_val_hex<T>(path: &Path) -> std::result::Result<T, ParseError>
where
	T: Num,
	ParseError: From<T::FromStrRadixErr>,
{
	read_sysfs_val_numeric::<T, 16>(path)
}

pub fn read_sysfs_val_dec<T>(path: &Path) -> std::result::Result<T, ParseError>
where
	T: Num,
	ParseError: From<T::FromStrRadixErr>,
{
	read_sysfs_val_numeric::<T, 10>(path)
}

fn read_sysfs_val_numeric<T, const R: u32>(path: &Path) -> std::result::Result<T, ParseError>
where
	T: Num,
	ParseError: From<T::FromStrRadixErr>,
{
	Ok(T::from_str_radix(&read_sysfs_val_string(path)?, R)?)
}
