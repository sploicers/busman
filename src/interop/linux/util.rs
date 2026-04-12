use num_traits::Num;

use crate::error::ParseError;
use std::{
	fs::{DirEntry, File},
	io::Read,
};

pub fn sysfs_val_hex<T>(entry: &DirEntry) -> std::result::Result<T, ParseError>
where
	T: Num,
	ParseError: From<T::FromStrRadixErr>,
{
	read_sysfs_val_numeric::<T, 16>(entry)
}

pub fn sysfs_val_dec<T>(entry: &DirEntry) -> std::result::Result<T, ParseError>
where
	T: Num,
	ParseError: From<T::FromStrRadixErr>,
{
	read_sysfs_val_numeric::<T, 10>(entry)
}

fn read_sysfs_val_numeric<T, const R: u32>(entry: &DirEntry) -> std::result::Result<T, ParseError>
where
	T: Num,
	ParseError: From<T::FromStrRadixErr>,
{
	let mut s = String::new();
	sysfs_file_handle(entry)?.read_to_string(&mut s)?;
	Ok(T::from_str_radix(s.trim(), R)?)
}

fn sysfs_file_handle(entry: &DirEntry) -> Result<File, ParseError> {
	Ok(File::open(entry.path()).inspect_err(|e| eprintln!("{}: {e:?}", entry.path().display()))?)
}
