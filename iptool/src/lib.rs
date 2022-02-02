#![deny(unsafe_op_in_unsafe_fn)]

use std::io::Result;
use std::net::Ipv4Addr;

mod iptool;

#[cfg(feature = "links")]
pub mod links;

#[doc(inline)]
pub use iptool::IpTool;

// Helper traits
pub trait IpAddrLinkExt
where
    Self: Sized,
{
    fn from_interface(dev: &str) -> Result<Self>;
}

impl IpAddrLinkExt for Ipv4Addr {
    fn from_interface(dev: &str) -> Result<Self> {
        let iptool = IpTool::new()?;

        iptool.get_address4(dev)
    }
}

pub trait MacAddrLinkExt: Sized {
    #[cfg(target_os = "linux")]
    fn from_interface(interface: &str) -> Result<Self>;
}

impl<T: From<[u8; 6]>> MacAddrLinkExt for T {
    #[cfg(target_os = "linux")]
    fn from_interface(interface: &str) -> Result<Self> {
        let tool = IpTool::new()?;

        Ok(tool.get_mac_data(interface)?.into())
    }
}
