use std::net::TcpStream;

use crate::result::Result;

use crate::{
	interop::linux::kernel::{KernelModule, load_kernel_module},
	protocol::USBDevice,
};

pub struct HostDriver {}

impl HostDriver {
	pub fn init() -> Result<Self> {
		load_kernel_module(KernelModule::UsbIpHost)?;
		todo!()
	}

	pub fn attach(&self, socket: TcpStream, device: USBDevice) -> Result<()> {
		todo!()
	}
}
