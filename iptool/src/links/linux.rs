use neli::consts::rtnl::{Arphrd, Ifla};
use std::any::Any;
use std::io::{Error, ErrorKind, Result};

use iptool_sys::RTNetlink;

pub struct LinkTool {
    sys: RTNetlink,
}

impl LinkTool {
    pub fn new() -> Result<Self> {
        Ok(Self {
            sys: RTNetlink::new()?,
        })
    }

    pub fn get_interfaces(&self) -> Result<Vec<Interface>> {
        let interfaces = self.sys.get_interfaces().map_err(|e| nl_error_to_io(e))?;
        let mut result = Vec::new();

        for interface in interfaces {
            result.push(Interface::try_from(interface)?)
        }

        Ok(result)
    }
}

#[derive(Debug)]
pub struct Interface {
    name: String,
    interface: iptool_sys::Interface,
}

impl Interface {
    // TODO: Result<String>?
    pub fn get_name(&self) -> &str {
        &self.name
    }

    pub fn print_info(&self) -> Result<String> {
        let index = self.interface.0.ifi_index;
        let name = self.get_name();

        let handle = self.interface.get_attr_handle();
        let mut attrs = Vec::new();

        if let Ok(mtu) = handle.get_attr_payload_as::<u32>(Ifla::Mtu) {
            attrs.push(format!("mtu {}", mtu));
        }

        if let Ok(qdisc) = handle.get_attr_payload_as_with_len::<String>(Ifla::Qdisc) {
            attrs.push(format!("qdisc {}", qdisc));
        }

        if let Ok(state) = handle.get_attr_payload_as(Ifla::Operstate) {
            attrs.push(format!("state {}", state_to_name(state)));
        }

        // TODO: mode

        if let Ok(group) = handle.get_attr_payload_as(Ifla::Group) {
            attrs.push(format!("group {}", group_to_name(group)));
        }

        if let Ok(qlen) = handle.get_attr_payload_as::<u32>(Ifla::Txqlen) {
            attrs.push(format!("qlen {}", qlen));
        }

        let link_type = type_to_name(self.interface.0.ifi_type);

        let mut addresses = Vec::new();

        if let Ok(Some(hwaddress)) = handle
            .get_attr_payload_as_with_len::<&[u8]>(Ifla::Address)
            .map(|a| print_mc_addr(a))
        {
            addresses.push(hwaddress);
        }

        if let Ok(Some(brd)) = handle
            .get_attr_payload_as_with_len(Ifla::Broadcast)
            .map(|a| print_mc_addr(a))
        {
            addresses.push(format!("brd {}", brd));
        }

        if let Ok(netns) = handle.get_attr_payload_as::<u32>(Ifla::LinkNetnsid) {
            // TODO: read name
            addresses.push(format!("link-netnsid {}", netns));
        }

        Ok(format!(
            "{index}: {name}: <FOO> {attrs}\n    link/{link_type} {address}",
            index = index,
            name = name,
            attrs = attrs.join(" "),
            link_type = link_type,
            address = addresses.join(" ")
        ))
    }
}

impl TryFrom<iptool_sys::Interface> for Interface {
    type Error = Error;

    fn try_from(interface: iptool_sys::Interface) -> std::result::Result<Self, Self::Error> {
        let name = interface.get_if_name().map_err(|e| nl_error_to_io(e))?;

        Ok(Self { name, interface })
    }
}

use crate::IpTool;
use neli::err::NlError;

fn nl_error_to_io<T, P>(error: NlError<T, P>) -> Error {
    match error {
        NlError::Nlmsgerr(e) => Error::from_raw_os_error(e.error),
        _ => Error::from(ErrorKind::Other),
    }
}

fn group_to_name(group: u32) -> &'static str {
    match group {
        0 => "default",
        // TODO: add more groups
        _ => "unknown",
    }
}

fn state_to_name(state: u8) -> &'static str {
    match state {
        2 => "DOWN",
        // TODO: add more states
        _ => "UNKNOWN",
    }
}

fn type_to_name(link_type: Arphrd) -> &'static str {
    match link_type {
        Arphrd::Loopback => "loopback",
        Arphrd::Ether => "ether",
        // TODO: add more types
        _ => "unknown",
    }
}

fn print_mc_addr(addr: &[u8]) -> Option<String> {
    if addr.len() != 6 {
        return None;
    }
    let mut hwprint = Vec::new();
    for byte in addr {
        hwprint.push(format!("{:02x}", byte));
    }
    Some(hwprint.join(":"))
}
