use std::{fmt::Display, process::ExitStatus};

use crate::platform::linux::kernel::KernelModule;

#[derive(Debug)]
pub enum PlatformError {
	CommandFailed(ExitStatus),
	ModuleNotFound(KernelModule),
}

impl core::error::Error for PlatformError {}

impl Display for PlatformError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{self:?}")
	}
}
