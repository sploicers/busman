use std::fs::File;
use std::io::Read;
use std::path::PathBuf;

use crate::error::{BusmanError, ParseError};
use crate::platform::PlatformError;
use crate::protocol::{DeviceSpeed, USBDevice};
use crate::result::Result;

const SYSFS_ROOT_VHCI: &str = "/sys/devices/platform/vhci_hcd.0";

const HUB_SPEED_CLASS_INDEX: usize = 0;
const VHCI_PORT_INDEX: usize = 1;
const DEVICE_STATUS_INDEX: usize = 2;

#[derive(Debug)]
pub struct Vhci {
	root: PathBuf,
}

impl Default for Vhci {
	fn default() -> Self {
		Self::new()
	}
}

impl Vhci {
	pub fn new() -> Self {
		Self {
			root: SYSFS_ROOT_VHCI.into(),
		}
	}

	pub fn import_device(&self, device: &USBDevice, fd: i32) -> Result<()> {
		let desired_speed_class = device.speed.port_class();

		if let Some(port) = self.get_available_port(desired_speed_class) {
			Ok(std::fs::write(
				self.attach_path(),
				format!("{} {} {} {}", port, fd, device.device_id(), device.speed as u32),
			)?)
		} else {
			Err(PlatformError::NoAvailableVhciPort.into())
		}
	}

	fn get_available_port(&self, desired_speed_class: USBPortSpeedClass) -> Option<u16> {
		let mut handle = File::open(self.status_path()).ok()?;
		let mut buf = String::new();
		handle.read_to_string(&mut buf).ok()?;

		buf.lines()
			.skip(1)
			.flat_map(VirtualPortStatusEntry::try_from)
			.find(|entry| entry.speed_class == desired_speed_class && entry.status == VirtualPortStatus::Available)
			.map(|entry| entry.port)
	}

	fn attach_path(&self) -> PathBuf {
		self.root.join("attach")
	}

	fn status_path(&self) -> PathBuf {
		self.root.join("status")
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum USBPortSpeedClass {
	HS,
	SS,
}

impl TryFrom<&str> for USBPortSpeedClass {
	type Error = BusmanError;

	fn try_from(value: &str) -> Result<Self> {
		match value {
			"hs" => Ok(Self::HS),
			"ss" => Ok(Self::SS),
			_ => Err(ParseError::InvalidSpeedClass.into()),
		}
	}
}

impl DeviceSpeed {
	pub fn port_class(self) -> USBPortSpeedClass {
		match self {
			Self::Super | Self::SuperPlus => USBPortSpeedClass::SS,
			_ => USBPortSpeedClass::HS,
		}
	}
}

#[derive(Debug, PartialEq, Eq)]
enum VirtualPortStatus {
	Available = 4, // SDEV_ST_AVAILABLE
	InUse = 5,     // SDEV_ST_USED
	Error = 6,     // SDEV_ST_ERROR
}

impl TryFrom<&str> for VirtualPortStatus {
	type Error = BusmanError;

	fn try_from(value: &str) -> Result<Self> {
		Ok(match value {
			"004" => Self::Available,
			"005" => Self::InUse,
			_ => Self::Error,
		})
	}
}

#[derive(Debug, PartialEq, Eq)]
struct VirtualPortStatusEntry {
	port: u16,
	speed_class: USBPortSpeedClass,
	status: VirtualPortStatus,
}

impl TryFrom<&str> for VirtualPortStatusEntry {
	type Error = BusmanError;

	fn try_from(value: &str) -> Result<Self> {
		let entry = value.split_whitespace().collect::<Vec<_>>();

		let speed_class = entry[HUB_SPEED_CLASS_INDEX].try_into()?;
		let port: u16 = entry[VHCI_PORT_INDEX].parse()?;
		let status = entry[DEVICE_STATUS_INDEX].try_into()?;

		Ok(Self {
			port,
			speed_class,
			status,
		})
	}
}
