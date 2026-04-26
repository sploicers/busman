use crate::protocol::USBDevice;
use crate::result::Result;

const VHCI_HCD_ATTACH: &str = "/sys/devices/platform/vhci_hcd/attach";

fn import_device(device: &USBDevice, fd: i32) -> Result<()> {
	std::fs::write(
		VHCI_HCD_ATTACH,
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
