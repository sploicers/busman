use std::{
	fs::{DirEntry, read_link},
	net::TcpStream,
	path::Path,
};

use crate::{
	error::ParseError,
	interop::linux::util::{sysfs_val_dec, sysfs_val_hex},
	protocol::{USBDevice, USBDeviceField, USBDeviceInterface, USBDeviceInterfaceField},
	result::Result,
};

const SYSFS_ROOT: &str = "/sys/bus/usb/devices";

pub fn list_devices() -> Result<Vec<USBDevice>> {
	let sysfs_device_entries = Path::new(SYSFS_ROOT)
		.read_dir()?
		.flatten()
		.filter(is_device_entry);

	let devices = sysfs_device_entries
		.filter_map(|entry| {
			USBDevice::try_from(entry)
				.inspect_err(|e| eprintln!("{e:?}"))
				.ok()
		})
		.collect();

	Ok(devices)
}

pub fn import_device(device: &USBDevice, socket: TcpStream) -> Result<()> {
	todo!()
}

fn is_device_entry(entry: &DirEntry) -> bool {
	entry
		.file_name()
		.to_str()
		.map_or(false, |name| !(name.contains(":") || name.contains("usb")))
}

impl TryFrom<DirEntry> for USBDevice {
	type Error = ParseError;

	fn try_from(entry: DirEntry) -> std::result::Result<Self, Self::Error> {
		if !entry.path().is_dir() {
			return Err(ParseError::NotADirectory);
		}

		let dirname = entry
			.file_name()
			.to_str()
			.ok_or(ParseError::NonUtf8Path)?
			.to_owned();

		let sysfs_subentries = entry.path().read_dir()?.flatten();
		let mut device = USBDevice::default();
		device.bus_id = dirname;

		for entry in sysfs_subentries {
			match USBDeviceField::from_sysfs_entry(&entry) {
				Some(field) => match field {
					USBDeviceField::BusNum => {
						device.bus_num = sysfs_val_dec(&entry)?;
					}
					USBDeviceField::DevNum => {
						device.device_num = sysfs_val_dec(&entry)?;
					}
					USBDeviceField::Speed => {
						device.speed = sysfs_val_dec(&entry)?;
					}
					USBDeviceField::VendorId => {
						device.vendor_id = sysfs_val_hex(&entry)?;
					}
					USBDeviceField::ProductId => {
						device.product_id = sysfs_val_hex(&entry)?;
					}
					USBDeviceField::RevisionNum => {
						device.revision_num = sysfs_val_dec(&entry)?;
					}
					USBDeviceField::Class => {
						device.class = sysfs_val_hex(&entry)?;
					}
					USBDeviceField::Subclass => {
						device.subclass = sysfs_val_hex(&entry)?;
					}
					USBDeviceField::Protocol => {
						device.protocol = sysfs_val_hex(&entry)?;
					}
					USBDeviceField::ConfigurationValue => {
						device.configuration_value = sysfs_val_dec(&entry)?;
					}
					USBDeviceField::NumConfigurations => {
						device.num_configurations = sysfs_val_dec(&entry)?;
					}
				},
				_ => {
					if let Ok(interface) = USBDeviceInterface::try_from(entry) {
						device.interfaces.push(interface);
					}
				}
			};
		}

		let device_path = read_link(entry.path())?.display().to_string();
		device.path = device_path;
		Ok(device)
	}
}

impl TryFrom<DirEntry> for USBDeviceInterface {
	type Error = ParseError;

	fn try_from(value: DirEntry) -> std::result::Result<Self, Self::Error> {
		// if !value.path().is_dir() {
		// 	return Err(ParseError::NotADirectory);
		// }

		// let dirname = value
		// 	.file_name()
		// 	.to_str()
		// 	.ok_or(ParseError::NonUtf8Path)?
		// 	.to_owned();

		// let is_interface = dirname.contains(':');
		// if !is_interface {
		// 	return Err(ParseError::NotAnInterface);
		// }

		let sysfs_subentries = value.path().read_dir()?.flatten();
		let mut interface = USBDeviceInterface::default();

		for entry in sysfs_subentries {
			if let Some(field) = USBDeviceInterfaceField::from_sysfs_entry(&entry) {
				match field {
					USBDeviceInterfaceField::Class => {
						interface.class = sysfs_val_hex(&entry)?;
					}
					USBDeviceInterfaceField::Subclass => {
						interface.subclass = sysfs_val_hex(&entry)?;
					}
					USBDeviceInterfaceField::Protocol => {
						interface.protocol = sysfs_val_hex(&entry)?;
					}
				}
			}
		}

		Ok(interface)
	}
}

impl USBDeviceField {
	fn from_sysfs_entry(entry: &DirEntry) -> Option<Self> {
		match entry.file_name().to_str() {
			Some(f) => match f {
				"busnum" => Some(Self::BusNum),
				"devnum" => Some(Self::DevNum),
				"speed" => Some(Self::Speed),
				"idVendor" => Some(Self::VendorId),
				"idProduct" => Some(Self::ProductId),
				"bcdDevice" => Some(Self::RevisionNum),
				"bDeviceClass" => Some(Self::Class),
				"bDeviceSubclass" => Some(Self::Subclass),
				"bDeviceProtocol" => Some(Self::Protocol),
				"bConfigurationValue" => Some(Self::ConfigurationValue),
				"bNumConfigurations" => Some(Self::NumConfigurations),
				_ => None, // The rest of the entries aren't part of the model as far as USBIP protocol is concerned
			},
			_ => None, // Skip any entries having non-utf8 names (shouldn't happen, sysfs file/dirnames are ASCII)
		}
	}
}

impl USBDeviceInterfaceField {
	fn from_sysfs_entry(entry: &DirEntry) -> Option<Self> {
		match entry.file_name().to_str() {
			Some(f) => match f {
				"bInterfaceClass" => Some(Self::Class),
				"bInterfaceSubclass" => Some(Self::Subclass),
				"bInterfaceProtocol" => Some(Self::Protocol),
				_ => None,
			},
			_ => None,
		}
	}
}
