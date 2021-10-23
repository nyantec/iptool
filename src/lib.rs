use std::io::{Error, Result};

#[cfg(target_family = "unix")]
use std::os::unix::io::{AsRawFd, FromRawFd, RawFd};

#[cfg(feature = "pnet")]
use pnet::datalink::MacAddr;

#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "linux")]
pub use linux::{Ifreq, SIOCGIFINDEX};

// TODO: macOS

/// Only use it for a short amount of time, as it does not close it's ioctl socket
pub struct IpTool {
    #[cfg(target_family = "unix")]
    fd: RawFd,
}

/*
TODO: is it already non send?
impl !Send for IpTool {}
impl !Sync for IpTool {}
 */

impl AsRawFd for IpTool {
    fn as_raw_fd(&self) -> RawFd {
        self.fd
    }
}

impl FromRawFd for IpTool {
    unsafe fn from_raw_fd(fd: RawFd) -> Self {
        Self { fd }
    }
}

// Helper function
#[allow(dead_code)] // not used yet on non linux
pub(crate) fn copy_slice(dst: &mut [u8], src: &[u8]) -> usize {
    let mut c = 0;

    for (d, s) in dst.iter_mut().zip(src.iter()) {
        *d = *s;
        c += 1;
    }

    c
}

pub fn parse_mac_addr(mac: &str) -> Result<[libc::c_char; 14]> {
    let mut addr: [libc::c_char; 14] = [0; 14];
    let mac_vec: Vec<&str> = mac.split(':').collect();
    if mac_vec.len() != 6 {
        // TODO: unlikly (https://doc.rust-lang.org/nightly/std/intrinsics/fn.unlikely.html)
        return Err(Error::from_raw_os_error(libc::EINVAL));
    }
    for x in 0..6 {
        if let Ok(data) = u8::from_str_radix(mac_vec[x], 16) {
            addr[x] = data as i8;
        } else {
            // TODO: unlikly (https://doc.rust-lang.org/nightly/std/intrinsics/fn.unlikely.html)
            return Err(Error::from_raw_os_error(libc::EINVAL));
        }
    }

    Ok(addr)
}

#[cfg(feature = "pnet")]
pub trait MacAddrLinxExt: From<[u8; 6]> {
    fn from_interface(interface: &str) -> Result<Self>;
}

#[cfg(feature = "pnet")]
impl MacAddrLinxExt for MacAddr {
    fn from_interface(interface: &str) -> Result<Self> {
        let tool = IpTool::new()?;

        let hwaddr = tool.get_mac_sa_data(interface)?;
        //let hwaddr: [u8; 6] = hwaddr.try_into()?;
        let hwaddr = unsafe { *(&hwaddr as *const _ as *const [u8; 6]) };

        let hwaddr: [u8; 6] = hwaddr.into();

        Ok(hwaddr.into())
    }
}

#[cold]
#[inline]
#[allow(dead_code)] // not used yet on non linux
/// Own (cold) function to optimize if statements
fn last_err() -> Error {
    Error::last_os_error()
}

#[cfg(test)]
mod test {

    #[test]
    #[allow(overflowing_literals)]
    fn parse_mac_addr() {
        let addr = "5A:E6:60:8F:5F:DE";
        let mut addr_vec: [libc::c_char; 14] = [0; 14];
        addr_vec[0] = 0x5A;
        addr_vec[1] = 0xE6;
        addr_vec[2] = 0x60;
        addr_vec[3] = 0x8F;
        addr_vec[4] = 0x5F;
        addr_vec[5] = 0xDE;

        assert_eq!(super::parse_mac_addr(addr).unwrap(), addr_vec);

        // not long enough address
        super::parse_mac_addr("5A:3B:2D").unwrap_err();
    }

    #[cfg(feature = "pnet")]
    #[test]
    fn macaddr_from_interface() {
        use super::MacAddrLinxExt;

        assert!(pnet::util::MacAddr::from_interface("lo").unwrap().is_zero());
    }
}
