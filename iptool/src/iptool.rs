use std::io::Result;
use std::net::Ipv4Addr;
use std::os::unix::io::AsRawFd;

#[cfg(target_family = "unix")]
use std::os::unix::io::RawFd;
#[cfg(target_family = "unix")]
use std::os::unix::prelude::FromRawFd;

use iptool_sys as ip_impl;

pub struct IpTool {
    inner: ip_impl::IpTool,
}

impl IpTool {
    pub fn new() -> Result<Self> {
        Ok(Self {
            inner: ip_impl::IpTool::new()?,
        })
    }

    pub fn set_up(&mut self, dev: &str, up: bool) -> Result<()> {
        self.inner.set_up(dev, up)
    }

    pub fn get_up(&self, dev: &str) -> Result<bool> {
        self.inner.get_up(dev)
    }

    pub fn set_mut(&mut self, dev: &str, mtu: u32) -> Result<()> {
        self.inner.set_mtu(dev, mtu)
    }

    pub fn get_mtu(&self, dev: &str) -> Result<u32> {
        self.inner.get_mtu(dev)
    }

    #[cfg(target_os = "linux")]
    pub fn get_index(&self, dev: &str) -> Result<i32> {
        self.inner.get_index(dev)
    }

    pub fn set_address4(
        &mut self,
        dev: &str,
        address: &Ipv4Addr,
        prefix_length: u32,
    ) -> Result<()> {
        self.inner
            .set_address(dev, &address.clone().into(), prefix_length)
    }

    pub fn get_address4(&self, dev: &str) -> Result<Ipv4Addr> {
        self.inner.get_address(dev)
    }

    pub fn set_mac(&mut self, dev: &str, mac: &str) -> Result<()> {
        self.inner.set_mac(dev, mac)
    }

    #[cfg(target_os = "linux")]
    pub fn set_mac_sa_data(&mut self, dev: &str, mac: [libc::c_char; 14]) -> Result<()> {
        self.inner.set_mac_sa_data(dev, mac)
    }

    #[cfg(target_os = "linux")]
    pub fn get_mac_sa_data(&self, dev: &str) -> Result<[libc::c_char; 14]> {
        self.inner.get_mac_sa_data(dev)
    }

    pub fn get_mac_data(&self, dev: &str) -> Result<[u8; 6]> {
        self.inner.get_mac_data(dev)
    }
}

#[cfg(target_family = "unix")]
impl AsRawFd for IpTool {
    fn as_raw_fd(&self) -> RawFd {
        self.inner.as_raw_fd()
    }
}

#[cfg(target_family = "unix")]
impl FromRawFd for IpTool {
    unsafe fn from_raw_fd(fd: RawFd) -> Self {
        Self {
            inner: unsafe { ip_impl::IpTool::from_raw_fd(fd) },
        }
    }
}
