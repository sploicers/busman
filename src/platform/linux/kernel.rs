use std::{fmt::Display, process::Command};

use crate::{platform::PlatformError, result::Result};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum KernelModule {
	VhciHcd,   // "vhci-hdc (client)"
	UsbIpHost, // "usbip-host (server)"
}

pub fn load_kernel_module(module: KernelModule) -> Result<()> {
	let exit_status = Command::new("/sbin/modprobe")
		.arg(module.to_string())
		.status()?;

	if !exit_status.success() {
		Err(PlatformError::CommandFailed(exit_status))?;
	}
	Ok(())
}

impl Display for KernelModule {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str(self.as_str())
	}
}

impl KernelModule {
	pub fn as_str(&self) -> &'static str {
		match self {
			KernelModule::VhciHcd => "vhci-hcd",
			KernelModule::UsbIpHost => "usbip-host",
		}
	}
}

impl AsRef<str> for KernelModule {
	fn as_ref(&self) -> &str {
		self.as_str()
	}
}
