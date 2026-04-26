use std::{
	io,
	net::TcpStream,
	os::fd::{AsFd, AsRawFd, IntoRawFd, OwnedFd},
	path::PathBuf,
	sync::Arc,
};

use crate::{
	connection::Connection,
	platform::linux::{
		kernel::{KernelModule, load_kernel_module},
		sysfs::{DeviceDir, InterfaceDir, SysfsHandle},
	},
	protocol::{USBDevice, USBDeviceField, USBDeviceInterface, USBDeviceInterfaceField},
	result::Result,
};

/// RAII wrapper representing an active export of a USB device by the USBIP host (machine running the server binary).
///
/// On acquision, it will:
/// * Write bus ID of selected device to `sys/bus/usb/drivers/usbip-host/match_busid`, which marks it as
/// managed by this driver, for when the kernel (usbip-host module) asks.
/// * Bind device, via write to `/sys/bus/usb/drivers/usbip-host/bind`
///
/// On client disconnect (or `drop()`), it will do the inverse:
/// * Unbind, via a write to `/sys/bus/usb/drivers/usbip-host/unbind`
/// * Remove device from allowlist (mark as not managed by usbip-host), via another write to `sys/bus/usb/drivers/usbip-host/match_busid`
///
pub struct DeviceExport {
	host: Arc<Host>,
	state: Option<InternalState>,
}

pub struct Host {
	sysfs: SysfsHandle,
}

struct InternalState {
	device: USBDevice,
	watchdog_fd: OwnedFd,
}

impl Host {
	pub fn new() -> Result<Self> {
		load_kernel_module(KernelModule::UsbIpCore)?;
		load_kernel_module(KernelModule::UsbIpHost)?;
		Ok(Self {
			sysfs: SysfsHandle::new(),
		})
	}

	/// Traverse sysfs device directory tree, constructing a `USBDevice` record for
	/// each listing.
	pub fn list_devices(&self) -> Result<Vec<USBDevice>> {
		Ok(self
			.sysfs
			.device_dirs()?
			.flat_map(|dir| {
				build_device(&dir).inspect_err(|e| {
					log::error!("Failed to build device: {e:?} from dir {dir:?}");
				})
			})
			.collect())
	}

	pub fn list_device_interfaces(&self, bus_id: &str) -> Result<Vec<USBDeviceInterface>> {
		Ok(if let Some(device) = self.sysfs.device_for_bus(bus_id)? {
			device
				.interface_dirs()?
				.flat_map(|dir| build_interface(&dir))
				.collect()
		} else {
			vec![]
		})
	}

	/// Build a `USBDevice` record via reading from sysfs directory tree, given bus ID of device
	pub fn device_by_id(&self, bus_id: &str) -> Result<Option<USBDevice>> {
		Ok(self
			.sysfs
			.device_for_bus(bus_id)?
			.and_then(|dir| build_device(&dir).ok()))
	}

	pub fn export_device(
		self: &Arc<Self>,
		conn: Connection,
		device: USBDevice,
	) -> Result<DeviceExport> {
		self.unbind_device_interfaces(&device)?;
		self.register_device(&device, KernelModule::UsbIpHost)?;
		self.bind_device(&device, KernelModule::UsbIpHost)?;

		let sock = conn.into_socket();
		let watchdog_fd = sock
			.as_fd()
			.try_clone_to_owned() // essentially a "dup" syscall
			.inspect_err(|e| log::error!("Failed to dup socket fd when exporting device: {e:?}"))?;

		self.handoff_device(&device, sock)?;

		Ok(DeviceExport {
			host: Arc::clone(self),
			state: Some(InternalState {
				device,
				watchdog_fd,
			}),
		})
	}

	pub fn release_device(&self, device: &USBDevice) -> Result<()> {
		self.unbind_device(device, KernelModule::UsbIpHost)
			.and_then(|_| self.unregister_device(device, KernelModule::UsbIpHost))
			.and_then(|_| self.trigger_sysfs_reprobe(device))
	}

	fn bind_device(&self, device: &USBDevice, driver: KernelModule) -> Result<()> {
		std::fs::write(self.driver_bind_path(driver), &device.bus_id)
			.inspect_err(|e| log::error!("Failed to bind device: {e:?}"))?;
		Ok(())
	}

	fn unbind_device<T: AsRef<str>>(&self, device: &USBDevice, driver: T) -> Result<()> {
		std::fs::write(self.driver_unbind_path(driver), &device.bus_id)
			.inspect_err(|e| log::error!("Failed to unbind device on disconnect: {e:?}"))?;
		Ok(())
	}

	fn register_device(&self, device: &USBDevice, driver: KernelModule) -> Result<()> {
		std::fs::write(
			self.driver_allowlist_path(driver),
			format!("add {}", &device.bus_id),
		)
		.inspect_err(|e| log::error!("Failed to add device to allowlist on export: {e:?}"))?;
		Ok(())
	}

	fn unregister_device(&self, device: &USBDevice, driver: KernelModule) -> Result<()> {
		std::fs::write(
			self.driver_allowlist_path(driver),
			format!("del {}", &device.bus_id),
		)
		.inspect_err(|e| log::error!("Failed to delete device from allowlist: {e:?}"))?;
		Ok(())
	}

	fn handoff_device(&self, device: &USBDevice, socket: TcpStream) -> Result<()> {
		std::fs::write(
			self.sock_assign_path(&device.bus_id),
			socket.into_raw_fd().to_string(),
		)
		.inspect_err(|e| log::error!("Failed to hand off device to kernel driver: {e:?}"))?;
		Ok(())
	}

	fn unbind_device_interfaces(&self, device: &USBDevice) -> Result<()> {
		if let Some(dir) = self.sysfs.device_for_bus(&device.bus_id)? {
			for dir in dir.interface_dirs()? {
				if let Ok(path) = dir.driver_dir()
					&& let Some(filename) = path.file_name()
				{
					std::fs::write(
						self.driver_unbind_path(filename.to_string_lossy()),
						&device.bus_id,
					)?;
				}
			}
		}
		Ok(())
	}

	fn trigger_sysfs_reprobe(&self, device: &USBDevice) -> Result<()> {
		std::fs::write(self.driver_reprobe_path(), &device.bus_id).inspect_err(|e| {
			log::error!(
				"Failed to trigger driver reprobe for device {}: {}",
				device.bus_id,
				e
			);
		})?;
		Ok(())
	}

	pub fn driver_bind_path<T: AsRef<str>>(&self, module: T) -> PathBuf {
		self.sysfs.driver_dir(module).join("bind")
	}

	pub fn driver_unbind_path<T: AsRef<str>>(&self, module: T) -> PathBuf {
		self.sysfs.driver_dir(module).join("unbind")
	}

	pub fn driver_allowlist_path<T: AsRef<str>>(&self, driver: T) -> PathBuf {
		self.sysfs.driver_dir(driver).join("match_busid")
	}

	pub fn sock_assign_path(&self, bus_id: &str) -> PathBuf {
		self.sysfs.devices_dir().join(bus_id).join("usbip_sockfd")
	}

	pub fn driver_reprobe_path(&self) -> PathBuf {
		self.sysfs.base_path().join("drivers_probe")
	}
}

impl DeviceExport {
	pub fn run_to_completion(mut self) -> Result<()> {
		let state = self.state.take().expect("");

		let mut poll_fd = libc::pollfd {
			fd: state.watchdog_fd.as_raw_fd(),
			events: libc::POLLRDHUP | libc::POLLHUP | libc::POLLERR,
			revents: 0,
		};

		let fd_count = 1; // Needed since first arg to underlying poll syscall is a C pointer
		let timeout = -1; // No timeout

		while let -1 = unsafe { libc::poll(&mut poll_fd, fd_count, timeout) } {
			if let Some(errno) = io::Error::last_os_error().raw_os_error() {
				// EINTR = system call interrupted, which is retryable. Other values of errno aren't.
				if errno != libc::EINTR {
					break;
				}
			}
		}
		self.host.release_device(&state.device)
	}
}

impl Drop for DeviceExport {
	fn drop(&mut self) {
		if let Some(state) = self.state.take()
			&& let Err(e) = self.host.release_device(&state.device)
		{
			log::warn!(
				"Failed to release device {} during Drop of DeviceExport: {}",
				state.device.bus_id,
				e
			)
		}
	}
}

fn build_device(dir: &DeviceDir) -> Result<USBDevice> {
	Ok(USBDevice {
		bus_id: dir.bus_id().to_owned(),
		path: "".into(),
		bus_num: dir.read_attr_dec(USBDeviceField::BusNum)?,
		device_num: dir.read_attr_dec(USBDeviceField::DevNum)?,
		speed: dir.read_attr_dec(USBDeviceField::Speed)?,
		vendor_id: dir.read_attr_hex(USBDeviceField::VendorId)?,
		product_id: dir.read_attr_hex(USBDeviceField::ProductId)?,
		revision_num: dir.read_attr_dec(USBDeviceField::RevisionNum)?,
		class: dir.read_attr_hex(USBDeviceField::Class)?,
		subclass: dir.read_attr_hex(USBDeviceField::Subclass)?,
		protocol: dir.read_attr_hex(USBDeviceField::Protocol)?,
		configuration_value: dir.read_attr_dec(USBDeviceField::ConfigurationValue)?,
		num_configurations: dir.read_attr_dec(USBDeviceField::NumConfigurations)?,
		interfaces: dir
			.interface_dirs()?
			.flat_map(|dir| build_interface(&dir))
			.collect(),
	})
}

fn build_interface(dir: &InterfaceDir) -> Result<USBDeviceInterface> {
	Ok(USBDeviceInterface {
		class: dir.read_attr_hex(USBDeviceInterfaceField::Class)?,
		subclass: dir.read_attr_hex(USBDeviceInterfaceField::Subclass)?,
		protocol: dir.read_attr_hex(USBDeviceInterfaceField::Protocol)?,
	})
}
