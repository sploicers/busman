use std::process::ExitStatus;

use crate::platform::linux::kernel::KernelModule;

#[derive(Debug)]
pub enum PlatformError {
	CommandFailed(ExitStatus),
	ModuleNotFound(KernelModule),
}
