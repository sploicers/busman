use crate::{
	interop::linux::kernel::{KernelModule, load_kernel_module},
	protocol::USBDevice,
};
use std::net::TcpStream;

use crate::result::Result;

pub struct VhciDriver {}

impl VhciDriver {
	pub fn init() -> Result<Self> {
		load_kernel_module(KernelModule::VhciHdc)?;
		Ok(Self {})
	}

	pub fn export(&self, socket: TcpStream, device: USBDevice) -> Result<()> {
		todo!()
	}
}
