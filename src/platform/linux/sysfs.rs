use num_traits::Num;
use std::{
	fs::DirEntry,
	num::ParseIntError,
	path::{Path, PathBuf},
};

use crate::{
	error::ParseError,
	protocol::{USBDeviceField, USBDeviceInterfaceField},
	result::Result,
};

const SYSFS_ROOT: &str = "/sys/bus/usb";

#[derive(Debug)]
pub struct SysfsHandle {
	path: PathBuf,
}

#[derive(Debug)]
pub struct DeviceDir {
	path: PathBuf,
	bus_id: String,
}

#[derive(Debug)]
pub struct InterfaceDir {
	path: PathBuf,
}

impl SysfsHandle {
	pub fn new() -> Self {
		Self {
			path: PathBuf::from(SYSFS_ROOT),
		}
	}

	pub fn base_path(&self) -> &Path {
		&self.path
	}

	pub fn device_dirs(&self) -> Result<impl Iterator<Item = DeviceDir>> {
		Ok(self
			.devices_dir()
			.read_dir()?
			.flatten()
			.filter_map(DeviceDir::from_dir_entry))
	}

	pub fn device_for_bus(&self, bus_id: &str) -> Result<Option<DeviceDir>> {
		let path = self.devices_dir().join(bus_id);

		Ok(if path.try_exists()? {
			DeviceDir::from_path(path)
		} else {
			None
		})
	}

	pub fn devices_dir(&self) -> PathBuf {
		self.path.join("devices")
	}

	pub fn driver_dir<T: AsRef<str>>(&self, driver: T) -> PathBuf {
		self.path.join("drivers").join(driver.as_ref())
	}
}

impl DeviceDir {
	pub fn from_path(path: PathBuf) -> Option<Self> {
		let dirname = path.file_name()?.to_str()?.to_owned();

		is_sysfs_device_dir(&dirname).then_some(Self {
			path,
			bus_id: dirname,
		})
	}

	pub fn from_dir_entry(dir: DirEntry) -> Option<Self> {
		Self::from_path(dir.path())
	}

	pub fn bus_id(&self) -> &str {
		&self.bus_id
	}

	pub fn interface_dirs(&self) -> Result<impl Iterator<Item = InterfaceDir>> {
		Ok(self
			.path
			.read_dir()?
			.flatten()
			.filter_map(InterfaceDir::new))
	}

	pub fn read_attr(&self, field: USBDeviceField) -> Result<String> {
		Ok(read_sysfs_val(&self.path.join(field.as_str()))?)
	}

	pub fn read_attr_hex<T>(&self, field: USBDeviceField) -> Result<T>
	where
		T: Num<FromStrRadixErr = ParseIntError>,
	{
		Ok(sysfs_val_hex(&self.path.join(field.as_str()))?)
	}

	pub fn read_attr_dec<T>(&self, field: USBDeviceField) -> Result<T>
	where
		T: Num<FromStrRadixErr = ParseIntError>,
	{
		Ok(sysfs_val_dec(&self.path.join(field.as_str()))?)
	}
}

impl InterfaceDir {
	pub fn new(dir: DirEntry) -> Option<Self> {
		let path = dir.path();
		let dirname = path.file_name()?.to_str()?;
		is_sysfs_interface_dir(dirname).then_some(InterfaceDir { path })
	}

	pub fn driver_dir(&self) -> Result<PathBuf> {
		Ok(self.path.join("driver").read_link()?)
	}

	pub fn read_attr(&self, field: USBDeviceInterfaceField) -> Result<String> {
		Ok(read_sysfs_val(&self.path.join(field.as_str()))?)
	}

	pub fn read_attr_hex<T>(&self, field: USBDeviceInterfaceField) -> Result<T>
	where
		T: Num<FromStrRadixErr = ParseIntError>,
	{
		Ok(sysfs_val_hex(&self.path.join(field.as_str()))?)
	}

	pub fn read_attr_dec<T>(&self, field: USBDeviceInterfaceField) -> Result<T>
	where
		T: Num<FromStrRadixErr = ParseIntError>,
	{
		Ok(sysfs_val_dec(&self.path.join(field.as_str()))?)
	}
}

fn is_sysfs_device_dir(name: &str) -> bool {
	// device dirs have naming patterns like "1-1", "2-1.4" - skip interfaces (":")
	// and root hubs ("usb[x]")
	!name.contains(':') && !name.starts_with("usb")
}

fn is_sysfs_interface_dir(name: &str) -> bool {
	// interface dirs have naming patterns like "1-1:1.0"
	name.contains(':')
}

fn sysfs_val_hex<T>(entry: &Path) -> std::result::Result<T, ParseError>
where
	T: Num,
	ParseError: From<T::FromStrRadixErr>,
{
	read_sysfs_val_numeric::<T, 16>(entry)
}

fn sysfs_val_dec<T>(path: &Path) -> std::result::Result<T, ParseError>
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
	Ok(T::from_str_radix(&read_sysfs_val(path)?, R)?)
}

fn read_sysfs_val(path: &Path) -> std::result::Result<String, ParseError> {
	let s = std::fs::read_to_string(path)?;
	Ok(s.trim().to_owned())
}
