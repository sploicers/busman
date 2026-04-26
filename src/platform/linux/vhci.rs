use std::path::PathBuf;

use crate::protocol::USBDevice;
use crate::result::Result;

const SYSFS_ROOT_VHCI: &str = "/sys/devices/platform/vhci_hcd";

#[derive(Debug)]
pub struct Vhci {
	root: PathBuf,
}

impl Vhci {
	pub fn new() -> Self {
		Self {
			root: SYSFS_ROOT_VHCI.into(),
		}
	}

	pub fn import_device(&self, device: &USBDevice, fd: i32) -> Result<()> {
		std::fs::write(
			self.attach_dir(),
			format!(
				"{} {} {} {}",
				fd,
				device.bus_id,
				device.device_id(),
				device.speed
			),
		)?;
		Ok(())
	}

	fn attach_dir(&self) -> PathBuf {
		self.root.join("attach")
	}
}
