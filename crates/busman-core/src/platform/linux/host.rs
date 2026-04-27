use std::{
	fs::DirEntry,
	io,
	net::TcpStream,
	num::ParseIntError,
	os::fd::{AsFd, AsRawFd, IntoRawFd, OwnedFd},
	path::PathBuf,
	sync::Arc,
};

use num_traits::Num;

use crate::{
	connection::Connection,
	platform::linux::{
		kernel::{KernelModule, load_kernel_module},
		sysfs::{read_sysfs_val_dec, read_sysfs_val_hex, read_sysfs_val_string, write_sysfs_val_string},
	},
	protocol::{USBDevice, USBDeviceField, USBDeviceInterface, USBDeviceInterfaceField},
	result::Result,
};

const SYSFS_ROOT_HOST: &str = "/sys/bus/usb";

pub struct Host {
	root: PathBuf,
}

#[derive(Debug)]
pub struct DeviceDir {
	path: PathBuf,
	bus_id: String,
}

#[derive(Debug)]
pub struct InterfaceDir {
	path: PathBuf,
}

/// RAII wrapper representing an active export of a USB device by the USBIP host (machine running the server binary).
///
/// On acquision, it will:
/// * Write bus ID of selected device to `sys/bus/usb/drivers/usbip-host/match_busid`, which marks it as
///   managed by this driver, for when the kernel (usbip-host module) asks.
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

struct InternalState {
	device: USBDevice,
	watchdog_fd: OwnedFd,
}

impl Host {
	pub fn new() -> Result<Self> {
		load_kernel_module(KernelModule::UsbIpCore)?;
		load_kernel_module(KernelModule::UsbIpHost)?;

		Ok(Self {
			root: SYSFS_ROOT_HOST.into(),
		})
	}

	/// Traverse sysfs device directory tree, constructing a `USBDevice` record for
	/// each listing.
	pub fn list_devices(&self) -> Result<Vec<USBDevice>> {
		Ok(self
			.device_dirs()?
			.flat_map(|dir| {
				build_device(&dir).inspect_err(|e| {
					log::error!("Failed to build device: {e:?} from dir {dir:?}");
				})
			})
			.collect())
	}

	pub fn list_device_interfaces(&self, bus_id: &str) -> Result<Vec<USBDeviceInterface>> {
		Ok(if let Some(device) = self.device_for_bus(bus_id)? {
			device.interface_dirs()?.flat_map(|dir| build_interface(&dir)).collect()
		} else {
			vec![]
		})
	}

	/// Build a `USBDevice` record via reading from sysfs directory tree, given bus ID of device
	pub fn device_by_id(&self, bus_id: &str) -> Result<Option<USBDevice>> {
		Ok(self.device_for_bus(bus_id)?.and_then(|dir| build_device(&dir).ok()))
	}

	pub fn export_device(self: &Arc<Self>, conn: Connection, device: USBDevice) -> Result<DeviceExport> {
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
			state: Some(InternalState { device, watchdog_fd }),
		})
	}

	pub fn release_device(&self, device: &USBDevice) -> Result<()> {
		self.unbind_device(device, KernelModule::UsbIpHost)
			.and_then(|_| self.unregister_device(device, KernelModule::UsbIpHost))
			.and_then(|_| self.trigger_sysfs_reprobe(device))
	}

	fn bind_device(&self, device: &USBDevice, driver: KernelModule) -> Result<()> {
		write_sysfs_val_string(&self.driver_bind_path(driver), &device.bus_id)
			.inspect_err(|e| log::error!("Failed to bind device: {e:?}"))?;
		Ok(())
	}

	fn unbind_device<T: AsRef<str>>(&self, device: &USBDevice, driver: T) -> Result<()> {
		write_sysfs_val_string(&self.driver_unbind_path(driver), &device.bus_id)
			.inspect_err(|e| log::error!("Failed to unbind device on disconnect: {e:?}"))?;
		Ok(())
	}

	fn register_device(&self, device: &USBDevice, driver: KernelModule) -> Result<()> {
		write_sysfs_val_string(&self.driver_allowlist_path(driver), format!("add {}", &device.bus_id))
			.inspect_err(|e| log::error!("Failed to add device to allowlist on export: {e:?}"))?;
		Ok(())
	}

	fn unregister_device(&self, device: &USBDevice, driver: KernelModule) -> Result<()> {
		write_sysfs_val_string(&self.driver_allowlist_path(driver), format!("del {}", &device.bus_id))
			.inspect_err(|e| log::error!("Failed to delete device from allowlist: {e:?}"))?;
		Ok(())
	}

	fn handoff_device(&self, device: &USBDevice, socket: TcpStream) -> Result<()> {
		write_sysfs_val_string(&self.sock_assign_path(&device.bus_id), socket.into_raw_fd().to_string())
			.inspect_err(|e| log::error!("Failed to hand off device to kernel driver: {e:?}"))?;
		Ok(())
	}

	fn unbind_device_interfaces(&self, device: &USBDevice) -> Result<()> {
		if let Some(dir) = self.device_for_bus(&device.bus_id)? {
			for dir in dir.interface_dirs()? {
				if let Ok(path) = dir.driver_dir()
					&& let Some(filename) = path.file_name()
				{
					write_sysfs_val_string(&self.driver_unbind_path(filename.to_string_lossy()), &device.bus_id)
						.inspect_err(|e| {
							log::error!(
								"Failed to unbind interface {filename:?} for device {}: {e:?}",
								device.bus_id,
							)
						})?;
				}
			}
		}
		Ok(())
	}

	fn trigger_sysfs_reprobe(&self, device: &USBDevice) -> Result<()> {
		write_sysfs_val_string(&self.driver_reprobe_path(), &device.bus_id).inspect_err(|e| {
			log::error!("Failed to trigger driver reprobe for device {}: {}", device.bus_id, e);
		})?;
		Ok(())
	}

	fn device_dirs(&self) -> Result<impl Iterator<Item = DeviceDir>> {
		Ok(self
			.devices_dir()
			.read_dir()?
			.flatten()
			.filter_map(DeviceDir::from_dir_entry))
	}

	fn device_for_bus(&self, bus_id: &str) -> Result<Option<DeviceDir>> {
		Ok(DeviceDir::from_path(self.devices_dir().join(bus_id)))
	}

	fn devices_dir(&self) -> PathBuf {
		self.root.join("devices")
	}

	pub fn driver_dir<T: AsRef<str>>(&self, driver: T) -> PathBuf {
		self.root.join("drivers").join(driver.as_ref())
	}

	fn driver_bind_path<T: AsRef<str>>(&self, module: T) -> PathBuf {
		self.driver_dir(module).join("bind")
	}

	fn driver_unbind_path<T: AsRef<str>>(&self, module: T) -> PathBuf {
		self.driver_dir(module).join("unbind")
	}

	fn driver_allowlist_path<T: AsRef<str>>(&self, driver: T) -> PathBuf {
		self.driver_dir(driver).join("match_busid")
	}

	fn sock_assign_path(&self, bus_id: &str) -> PathBuf {
		self.devices_dir().join(bus_id).join("usbip_sockfd")
	}

	fn driver_reprobe_path(&self) -> PathBuf {
		self.root.join("drivers_probe")
	}
}

impl DeviceDir {
	pub fn from_path(path: PathBuf) -> Option<Self> {
		let bus_id = path.file_name()?.to_str()?.to_owned();
		is_device_name(&bus_id).then_some(Self { path, bus_id })
	}

	pub fn from_dir_entry(dir: DirEntry) -> Option<Self> {
		Self::from_path(dir.path())
	}

	pub fn bus_id(&self) -> &str {
		&self.bus_id
	}

	pub fn interface_dirs(&self) -> Result<impl Iterator<Item = InterfaceDir>> {
		Ok(self.path.read_dir()?.flatten().filter_map(InterfaceDir::from_dir_entry))
	}

	pub fn read_attr(&self, field: USBDeviceField) -> Result<String> {
		Ok(read_sysfs_val_string(&self.path.join(field.as_str()))?)
	}

	pub fn read_attr_hex<T>(&self, field: USBDeviceField) -> Result<T>
	where
		T: Num<FromStrRadixErr = ParseIntError>,
	{
		Ok(read_sysfs_val_hex(&self.path.join(field.as_str()))?)
	}

	pub fn read_attr_dec<T>(&self, field: USBDeviceField) -> Result<T>
	where
		T: Num<FromStrRadixErr = ParseIntError>,
	{
		Ok(read_sysfs_val_dec(&self.path.join(field.as_str()))?)
	}
}

impl InterfaceDir {
	pub fn from_path(path: PathBuf) -> Option<Self> {
		let name = path.file_name()?.to_str()?;
		is_interface_name(name).then_some(Self { path })
	}

	pub fn from_dir_entry(dir: DirEntry) -> Option<Self> {
		Self::from_path(dir.path())
	}

	fn driver_dir(&self) -> Result<PathBuf> {
		Ok(self.path.join("driver").read_link()?)
	}

	pub fn read_attr(&self, field: USBDeviceInterfaceField) -> Result<String> {
		Ok(read_sysfs_val_string(&self.path.join(field.as_str()))?)
	}

	pub fn read_attr_hex<T>(&self, field: USBDeviceInterfaceField) -> Result<T>
	where
		T: Num<FromStrRadixErr = ParseIntError>,
	{
		Ok(read_sysfs_val_hex(&self.path.join(field.as_str()))?)
	}

	pub fn read_attr_dec<T>(&self, field: USBDeviceInterfaceField) -> Result<T>
	where
		T: Num<FromStrRadixErr = ParseIntError>,
	{
		Ok(read_sysfs_val_dec(&self.path.join(field.as_str()))?)
	}
}

impl DeviceExport {
	pub fn run_to_completion(mut self) -> Result<()> {
		if let Some(state) = self.state.take() {
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
		} else {
			Ok(())
		}
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
		interfaces: dir.interface_dirs()?.flat_map(|dir| build_interface(&dir)).collect(),
	})
}

fn build_interface(dir: &InterfaceDir) -> Result<USBDeviceInterface> {
	Ok(USBDeviceInterface {
		class: dir.read_attr_hex(USBDeviceInterfaceField::Class)?,
		subclass: dir.read_attr_hex(USBDeviceInterfaceField::Subclass)?,
		protocol: dir.read_attr_hex(USBDeviceInterfaceField::Protocol)?,
	})
}

fn is_device_name(name: &str) -> bool {
	!name.contains(':') && !name.starts_with("usb")
}

fn is_interface_name(name: &str) -> bool {
	name.contains(':')
}
