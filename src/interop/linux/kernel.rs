use std::{fmt::Display, process::Command};

use crate::result::Result;

pub(crate) enum KernelModule {
	VhciHdc,   // "vhci-hdc (client)"
	UsbIpHost, // "usbip-host (server)"
}

pub fn load_kernel_module(module: KernelModule) -> Result<()> {
	Command::new("/sbin/modprobe")
		.arg(module.to_string())
		.status()?;

	Ok(())
}

impl Display for KernelModule {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str(match self {
			KernelModule::VhciHdc => "vhci-hdc",
			KernelModule::UsbIpHost => "usbip-host",
		})
	}
}
