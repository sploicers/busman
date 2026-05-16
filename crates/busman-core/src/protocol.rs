use std::fmt::Display;
use std::io::{Read, Write};

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use num_enum::{IntoPrimitive, TryFromPrimitive};

use crate::error::{BusmanError, DecodeError, EncodeError, ParseError};
use crate::result::{DecodeResult, EncodeResult, Result};

pub trait Encode {
	fn encode(&self, writer: &mut impl Write) -> EncodeResult<()>;
}

pub trait Decode: Sized {
	fn decode(reader: &mut impl Read) -> DecodeResult<Self>;
}

#[derive(Debug)]
pub enum Frame {
	RequestDeviceList(PayloadRequestDeviceList),     // OP_REQ_DEVLIST
	ReplyDeviceList(PayloadReplyDeviceList),         // OP_REP_DEVLIST
	RequestDeviceImport(PayloadRequestDeviceImport), // OP_REQ_IMPORT
	ReplyDeviceImport(PayloadReplyDeviceImport),     // OP_REP_IMPORT
}

#[repr(u16)]
#[derive(Debug)]
enum Opcode {
	ReqDevlist = 0x8005,
	RepDevlist = 0x0005,
	ReqImport = 0x8003,
	RepImport = 0x0003,
}

#[derive(Clone, Debug, Default)]
pub struct USBDevice {
	pub path: String,
	pub bus_id: String,
	pub bus_num: u32,
	pub device_num: u32,
	pub speed: DeviceSpeed,
	pub vendor_id: u16,
	pub product_id: u16,
	pub revision_num: u16, // called "bcdDevice" in the protocol spec
	pub class: u8,
	pub subclass: u8,
	pub protocol: u8,
	pub configuration_value: u8,
	pub num_configurations: u8,
	pub interfaces: Vec<USBDeviceInterface>,
}

impl USBDevice {
	pub fn device_id(&self) -> u32 {
		(self.bus_num << 16) | self.device_num
	}
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, IntoPrimitive, TryFromPrimitive)]
#[repr(u32)]
pub enum DeviceSpeed {
	#[default]
	Unknown = 0,
	Low = 1,
	Full = 2,
	High = 3,
	Wireless = 4,
	Super = 5,
	SuperPlus = 6,
}

impl TryFrom<String> for DeviceSpeed {
	type Error = BusmanError;

	fn try_from(value: String) -> Result<Self> {
		match value.trim() {
			"1.5" => Ok(Self::Low),
			"12" => Ok(Self::Full),
			"480" => Ok(Self::High),
			"5000" => Ok(Self::Super),
			"10000" | "20000" => Ok(Self::SuperPlus),
			_ => Err(ParseError::InvalidSpeedClass.into()),
		}
	}
}

#[derive(Clone, Debug, Default)]
pub struct USBDeviceInterface {
	pub class: u8,
	pub subclass: u8,
	pub protocol: u8,
}

#[derive(Debug)]
pub enum USBDeviceInterfaceField {
	Class,
	Subclass,
	Protocol,
}

impl USBDeviceInterfaceField {
	pub fn as_str(&self) -> &str {
		match self {
			USBDeviceInterfaceField::Class => "bInterfaceClass",
			USBDeviceInterfaceField::Subclass => "bInterfaceClass",
			USBDeviceInterfaceField::Protocol => "bInterfaceProtocol",
		}
	}
}

#[derive(Debug)]
pub enum USBDeviceField {
	BusNum,
	DevNum,
	Speed,
	VendorId,
	ProductId,
	RevisionNum,
	Class,
	Subclass,
	Protocol,
	ConfigurationValue,
	NumConfigurations,
}

impl USBDeviceField {
	pub fn as_str(&self) -> &str {
		match self {
			USBDeviceField::BusNum => "busnum",
			USBDeviceField::DevNum => "devnum",
			USBDeviceField::Speed => "speed",
			USBDeviceField::VendorId => "idVendor",
			USBDeviceField::ProductId => "idProduct",
			USBDeviceField::RevisionNum => "bcdDevice",
			USBDeviceField::Class => "bDeviceClass",
			USBDeviceField::Subclass => "bDeviceSubClass",
			USBDeviceField::Protocol => "bDeviceProtocol",
			USBDeviceField::ConfigurationValue => "bConfigurationValue",
			USBDeviceField::NumConfigurations => "bNumConfigurations",
		}
	}
}

// Empty - in the protocol spec there're "version" and "status" fields,
// but we don't care about version and status is defined as always zero
// for request payloads
#[derive(Debug)]
pub struct PayloadRequestDeviceList;

#[derive(Debug)]
pub struct PayloadReplyDeviceList {
	pub status: u32,
	pub devices: Vec<USBDevice>,
}

#[derive(Debug)]
pub struct PayloadRequestDeviceImport {
	pub bus_id: String,
}

#[derive(Debug)]
pub struct PayloadReplyDeviceImport {
	pub status: u32,
	pub device: Option<USBDevice>,
}

impl TryFrom<u16> for Opcode {
	type Error = DecodeError;

	fn try_from(value: u16) -> core::result::Result<Self, Self::Error> {
		match value {
			val if val == Self::ReqDevlist as u16 => Ok(Self::ReqDevlist),
			val if val == Self::RepDevlist as u16 => Ok(Self::RepDevlist),
			val if val == Self::ReqImport as u16 => Ok(Self::ReqImport),
			val if val == Self::RepImport as u16 => Ok(Self::RepImport),

			// We don't implement the rest of the spec - only establishing TCP conn in userland
			// after selecting and importing a device, and then handing fd of the socket off to the kernel
			// afterwards for the rest
			val => Err(DecodeError::UnsupportedOpcode(val)),
		}
	}
}

impl Display for Frame {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str(match self {
			Frame::RequestDeviceList(_) => "OP_REQ_DEVLIST",
			Frame::ReplyDeviceList(_) => "OP_REP_DEVLIST",
			Frame::RequestDeviceImport(_) => "OP_REQ_IMPORT",
			Frame::ReplyDeviceImport(_) => "OP_REP_IMPORT",
		})
	}
}

impl Encode for Frame {
	fn encode(&self, writer: &mut impl Write) -> EncodeResult<()> {
		match self {
			Frame::RequestDeviceList(data) => data.encode(writer),
			Frame::ReplyDeviceList(data) => data.encode(writer),
			Frame::RequestDeviceImport(data) => data.encode(writer),
			Frame::ReplyDeviceImport(data) => data.encode(writer),
		}
	}
}

impl Decode for Frame {
	fn decode(reader: &mut impl Read) -> DecodeResult<Self> {
		reader.read_u16::<BigEndian>()?; // Skip past USBIP protocol version, don't care about it
		let opcode = Opcode::try_from(reader.read_u16::<BigEndian>()?)?;

		Ok(match opcode {
			Opcode::ReqDevlist => Self::RequestDeviceList(PayloadRequestDeviceList::decode(reader)?),
			Opcode::RepDevlist => Self::ReplyDeviceList(PayloadReplyDeviceList::decode(reader)?),
			Opcode::ReqImport => Self::RequestDeviceImport(PayloadRequestDeviceImport::decode(reader)?),
			Opcode::RepImport => Self::ReplyDeviceImport(PayloadReplyDeviceImport::decode(reader)?),
		})
	}
}

impl Encode for PayloadRequestDeviceList {
	fn encode(&self, writer: &mut impl Write) -> EncodeResult<()> {
		writer.write_u16::<BigEndian>(0)?; // version
		writer.write_u16::<BigEndian>(Opcode::ReqDevlist as u16)?;
		writer.write_u32::<BigEndian>(0)?; // status - spec specifies as "unused, always zero"
		Ok(())
	}
}

impl Decode for PayloadRequestDeviceList {
	fn decode(reader: &mut impl Read) -> DecodeResult<Self> {
		_ = reader.read_u32::<BigEndian>()?; // status
		Ok(Self)
	}
}

impl Encode for PayloadReplyDeviceList {
	fn encode(&self, writer: &mut impl Write) -> EncodeResult<()> {
		writer.write_u16::<BigEndian>(0)?; // version
		writer.write_u16::<BigEndian>(Opcode::RepDevlist as u16)?;
		writer.write_u32::<BigEndian>(self.status)?;
		writer.write_u32::<BigEndian>(self.devices.len() as u32)?;

		for device in &self.devices {
			device.encode(writer)?;
		}
		Ok(())
	}
}

impl Decode for PayloadReplyDeviceList {
	fn decode(reader: &mut impl Read) -> DecodeResult<Self> {
		let status = reader.read_u32::<BigEndian>()?;
		let device_count = reader.read_u32::<BigEndian>()?;

		let mut devices = Vec::with_capacity(device_count as usize);
		for _ in 0..device_count {
			devices.push(USBDevice::decode(reader)?);
		}

		Ok(Self { status, devices })
	}
}

impl Encode for PayloadRequestDeviceImport {
	fn encode(&self, writer: &mut impl Write) -> EncodeResult<()> {
		writer.write_u16::<BigEndian>(0)?; // version
		writer.write_u16::<BigEndian>(Opcode::ReqImport as u16)?; // opcode
		writer.write_u32::<BigEndian>(0)?; // status
		writer.write_all(&fixed_length_buffer_from_str::<32>(&self.bus_id)?)?;
		Ok(())
	}
}

impl Decode for PayloadRequestDeviceImport {
	fn decode(reader: &mut impl Read) -> DecodeResult<Self> {
		reader.read_u32::<BigEndian>()?; // status
		let bus_id = string_from_fixed_length_buffer::<32>(reader)?;
		Ok(Self { bus_id })
	}
}

impl Encode for PayloadReplyDeviceImport {
	fn encode(&self, writer: &mut impl Write) -> EncodeResult<()> {
		writer.write_u16::<BigEndian>(0)?; // version
		writer.write_u16::<BigEndian>(Opcode::RepImport as u16)?;
		writer.write_u32::<BigEndian>(self.status)?;

		if let Some(device) = &self.device
			&& self.status == 0
		{
			device.encode(writer)?;
		}
		Ok(())
	}
}

impl Decode for PayloadReplyDeviceImport {
	fn decode(reader: &mut impl Read) -> DecodeResult<Self> {
		let status_code = reader.read_u32::<BigEndian>()?;

		Ok(Self {
			status: status_code,
			device: if status_code == 0 {
				Some(USBDevice::decode(reader)?)
			} else {
				None
			},
		})
	}
}

impl Encode for USBDevice {
	fn encode(&self, writer: &mut impl Write) -> EncodeResult<()> {
		writer.write_all(&fixed_length_buffer_from_str::<256>(&self.path)?)?;
		writer.write_all(&fixed_length_buffer_from_str::<32>(&self.bus_id)?)?;
		writer.write_u32::<BigEndian>(self.bus_num)?;
		writer.write_u32::<BigEndian>(self.device_num)?;
		writer.write_u32::<BigEndian>(self.speed as u32)?;
		writer.write_u16::<BigEndian>(self.vendor_id)?;
		writer.write_u16::<BigEndian>(self.product_id)?;
		writer.write_u16::<BigEndian>(self.revision_num)?;
		writer.write_u8(self.class)?;
		writer.write_u8(self.subclass)?;
		writer.write_u8(self.protocol)?;
		writer.write_u8(self.configuration_value)?;
		writer.write_u8(self.num_configurations)?;
		writer.write_u8(self.interfaces.len() as u8)?;

		for interface in &self.interfaces {
			interface.encode(writer)?;
		}
		Ok(())
	}
}

impl Decode for USBDevice {
	fn decode(reader: &mut impl Read) -> DecodeResult<Self> {
		let path = string_from_fixed_length_buffer::<256>(reader)?;
		let bus_id = string_from_fixed_length_buffer::<32>(reader)?;
		let bus_num = reader.read_u32::<BigEndian>()?;
		let device_num = reader.read_u32::<BigEndian>()?;
		let speed = DeviceSpeed::try_from_primitive(reader.read_u32::<BigEndian>()?)?;
		let vendor_id = reader.read_u16::<BigEndian>()?;
		let product_id = reader.read_u16::<BigEndian>()?;
		let revision_num = reader.read_u16::<BigEndian>()?;
		let class = reader.read_u8()?;
		let subclass = reader.read_u8()?;
		let protocol = reader.read_u8()?;
		let configuration_value = reader.read_u8()?;
		let num_configurations = reader.read_u8()?;
		let num_interfaces = reader.read_u8()?;

		let mut interfaces = Vec::with_capacity(num_interfaces as usize);
		for _ in 0..num_interfaces {
			interfaces.push(USBDeviceInterface::decode(reader)?);
		}

		Ok(Self {
			path,
			bus_id,
			bus_num,
			device_num,
			speed,
			vendor_id,
			product_id,
			revision_num,
			class,
			subclass,
			protocol,
			configuration_value,
			num_configurations,
			interfaces,
		})
	}
}

impl Encode for USBDeviceInterface {
	fn encode(&self, writer: &mut impl Write) -> EncodeResult<()> {
		writer.write_u8(self.class)?;
		writer.write_u8(self.subclass)?;
		writer.write_u8(self.protocol)?;
		writer.write_u8(0)?; // protocol spec specifies this is just padding for alignment, and always zero
		Ok(())
	}
}

impl Decode for USBDeviceInterface {
	fn decode(reader: &mut impl Read) -> DecodeResult<Self> {
		let class = reader.read_u8()?;
		let subclass = reader.read_u8()?;
		let protocol = reader.read_u8()?;
		reader.read_u8()?; // padding

		Ok(Self {
			class,
			subclass,
			protocol,
		})
	}
}

fn string_from_fixed_length_buffer<const N: usize>(reader: &mut impl Read) -> DecodeResult<String> {
	let mut buf: [u8; N] = [0; N];
	reader.read_exact(&mut buf)?;

	let null_byte_pos = buf.iter().position(|&b| b == b'\0').unwrap_or(buf.len());
	let s = str::from_utf8(&buf[..null_byte_pos])?;
	Ok(s.to_owned())
}

fn fixed_length_buffer_from_str<const N: usize>(s: &str) -> EncodeResult<[u8; N]> {
	if s.len() >= N {
		// >= here is because we need space for null-terminator
		Err(EncodeError::StringExceedsFieldWidth(s.to_owned(), N))?;
	}

	let mut buf: [u8; N] = [0; N];
	buf[..s.len()].copy_from_slice(s.as_bytes());
	Ok(buf)
}
