use std::io::{Error, Result};
use std::net::{IpAddr, Ipv4Addr};

use libc::{
    c_int, c_short, c_uchar, close, in6_addr, ioctl, sockaddr, sockaddr_in, sockaddr_in6, socket,
};

use super::{copy_slice, last_err, parse_mac_addr, IpTool};

pub const SIOCGIFINDEX: u64 = 0x8933;

impl IpTool {
    pub fn new() -> Result<Self> {
        let fd = Self::get_ctl_fd()?;

        Ok(Self { fd })
    }

    pub fn set_up(&self, dev: &str, up: bool) -> Result<()> {
        let mut ifr = Ifreq::new(dev);

        ifr.ioctl(&self, libc::SIOCGIFFLAGS)?;

        let flag_val = libc::IFF_UP as i16;

        // SAFETY: union
        unsafe {
            ifr.ifr_ifru.ifru_flags = if up {
                ifr.ifr_ifru.ifru_flags | flag_val
            } else {
                ifr.ifr_ifru.ifru_flags & (!flag_val)
            };
        }

        ifr.ioctl(&self, libc::SIOCSIFFLAGS)?;

        if self.get_up(dev)? != up {
            return Err(Error::from_raw_os_error(libc::ENOTRECOVERABLE));
        }

        Ok(())
    }

    pub fn get_up(&self, dev: &str) -> Result<bool> {
        let mut ifr = Ifreq::new(dev);

        ifr.ioctl(&self, libc::SIOCGIFFLAGS)?;

        // unions
        let flags: i16 = unsafe { ifr.ifr_ifru.ifru_flags };
        Ok((flags & libc::IFF_UP as i16) == 1)
    }

    pub fn set_mtu(&self, dev: &str, mtu: u32) -> Result<()> {
        let mut ifr = Ifreq::new(dev);
        ifr.ifr_ifru.ifru_mtu = mtu as i32;

        ifr.ioctl(&self, libc::SIOCSIFMTU)?;

        Ok(())
    }

    pub fn get_mtu(&self, dev: &str) -> Result<u32> {
        let mut ifr = Ifreq::new(dev);

        ifr.ioctl(&self, libc::SIOCGIFMTU)?;

        let mtu = unsafe { ifr.ifr_ifru.ifru_mtu as u32 };
        Ok(mtu)
    }

    pub fn get_index(&self, dev: &str) -> Result<c_int> {
        let mut ifr = Ifreq::new(dev);

        ifr.ioctl(&self, SIOCGIFINDEX)?;

        Ok(unsafe { ifr.ifr_ifru.ifru_ivalue })
    }

    pub fn set_address(&self, dev: &str, address: &IpAddr, prefix_length: u32) -> Result<()> {
        let index = self.get_index(dev)?;
        match address {
            IpAddr::V4(addr) => {
                // TODO: ipv4
                if prefix_length > 32 {
                    return Err(Error::from_raw_os_error(libc::EINVAL));
                }

                let mut ifr = Ifreq::new(dev);
                ifr.ifr_ifru.ifru_addr_v4.sin_family = libc::AF_INET as _;
                ifr.ifr_ifru.ifru_addr_v4.sin_addr.s_addr = u32::from_ne_bytes(addr.octets());

                ifr.ioctl(&self, libc::SIOCSIFADDR)?;

                ifr.ifr_ifru.ifru_addr_v4.sin_addr.s_addr = u32::MAX >> (32 - prefix_length);

                ifr.ioctl(&self, libc::SIOCSIFNETMASK)?;
            }
            IpAddr::V6(addr) => {
                let mut ifr = Ifreq6 {
                    prefix_length,
                    ifindex: index as _,
                    addr: in6_addr {
                        s6_addr: addr.octets(),
                    },
                };
                ifr.ioctl(&self, libc::SIOCSIFADDR)?;
            }
        }

        Ok(())
        //let mut ifr = Ifreq::new(dev);
        /*match address {
            IpAddr::V4(addr) => {
                ifr.ifr_ifru.ifru_addr_v4.sin_family = libc::AF_INET as libc::sa_family_t;
                ifr.ifr_ifru.ifru_addr_v4.sin_addr.s_addr = u32::from_ne_bytes(addr.octets());
            }
        }*/

        /*let res = unsafe { libc::ioctl(self.fd, libc::SIOCSIFADDR as _, &mut ifr) };*/
    }

    pub fn get_address(&self, _dev: &str) -> Result<IpAddr> {
        // TODO
        Err(Error::from_raw_os_error(libc::ENOSYS))
    }

    pub fn set_mac(&self, dev: &str, mac: &str) -> Result<()> {
        self.set_mac_sa_data(dev, parse_mac_addr(mac)?)
    }
    pub fn set_mac_sa_data(&self, dev: &str, mac: [libc::c_char; 14]) -> Result<()> {
        let mut ifr = Ifreq::new(dev);
        ifr.ifr_ifru.ifru_hwaddr.sa_family = libc::ARPHRD_ETHER;
        ifr.ifr_ifru.ifru_hwaddr.sa_data = mac;

        ifr.ioctl(&self, libc::SIOCSIFHWADDR)
    }

    pub fn get_mac_sa_data(&self, dev: &str) -> Result<[libc::c_char; 14]> {
        let mut ifr = Ifreq::new(dev);
        ifr.ifr_ifru.ifru_hwaddr.sa_family = libc::ARPHRD_ETHER;

        ifr.ioctl(&self, libc::SIOCGIFHWADDR)?;

        let sa_data = unsafe { ifr.ifr_ifru.ifru_hwaddr.sa_data };
        Ok(sa_data)
    }
    // TODO: get_mac -> String

    fn get_ctl_fd() -> Result<c_int> {
        let fd = unsafe { socket(libc::PF_INET, libc::SOCK_DGRAM, 0) };
        if fd >= 0 {
            return Ok(fd);
        }
        let error = std::io::Error::last_os_error();
        let fd = unsafe { socket(libc::PF_PACKET, libc::SOCK_DGRAM, 0) };
        if fd >= 0 {
            return Ok(fd);
        }
        let fd = unsafe { socket(libc::PF_INET6, libc::SOCK_DGRAM, 0) };
        if fd >= 0 {
            return Ok(fd);
        }
        Err(error)
    }
}

impl Drop for IpTool {
    fn drop(&mut self) {
        unsafe { close(self.fd) };
    }
}

#[repr(C)]
union IfrIfru {
    ifru_addr: sockaddr,
    ifru_hwaddr: sockaddr,
    ifru_addr_v4: sockaddr_in,
    ifru_addr_v6: sockaddr_in6,
    ifru_dstaddr: sockaddr,
    ifru_broadaddr: sockaddr,
    ifru_flags: c_short,
    ifru_metric: c_int,
    ifru_ivalue: c_int,
    ifru_mtu: c_int,
    ifru_phys: c_int,
    ifru_media: c_int,
    ifru_intval: c_int,
    //ifru_data: caddr_t,
    //ifru_devmtu: ifdevmtu,
    //ifru_kpi: ifkpi,
    ifru_wake_flags: u32,
    ifru_route_refcnt: u32,
    ifru_cap: [c_int; 2],
    ifru_functional_type: u32,
}

trait IoctlReq {
    fn ioctl(&mut self, iptool: &IpTool, request: libc::c_ulong) -> Result<()> {
        let res = unsafe { ioctl(iptool.fd, request as _, self) };
        if res < 0 {
            Err(last_err())
        } else {
            Ok(())
        }
    }
}

#[repr(C)]
pub struct Ifreq {
    ifr_name: [c_uchar; libc::IFNAMSIZ],
    ifr_ifru: IfrIfru,
}

impl Ifreq {
    pub fn new(dev: &str) -> Self {
        //let mut ifr_name = [0; libc::IF_NAMESIZE];

        //ifr_name[..dev.len()].copy_from_slice(dev.as_bytes().as_ref());

        let s: [u8; core::mem::size_of::<Self>()] = [0; core::mem::size_of::<Self>()];
        let mut s: Self = unsafe { core::mem::transmute(s) };

        copy_slice(&mut s.ifr_name, dev.as_bytes());

        s
        /*Self {
            ifr_name,
            ifr_ifru: IfrIfru { ifru_flags: 0 },
        }*/
    }
}

impl IoctlReq for Ifreq {}

#[repr(C)]
pub struct Ifreq6 {
    addr: in6_addr,
    prefix_length: u32,
    ifindex: libc::c_uint,
}

impl IoctlReq for Ifreq6 {}

#[cfg(test)]
mod test {
    use super::IpTool;
    use std::net::{IpAddr, Ipv4Addr};

    const TEST_INTERFACE: &str = "loop1";
    #[test]
    #[ignore]
    fn down() {
        let ip_tool = IpTool::new().unwrap();

        ip_tool.set_up(TEST_INTERFACE, false).unwrap();
    }

    #[test]
    #[ignore]
    fn up() {
        let ip_tool = IpTool::new().unwrap();

        ip_tool.set_up(TEST_INTERFACE, true).unwrap();
    }

    #[test]
    #[ignore]
    fn sleep_down_and_up() {
        let ip_tool = IpTool::new().unwrap();

        ip_tool.set_up(TEST_INTERFACE, false).unwrap();

        std::thread::sleep(std::time::Duration::from_secs(5));

        ip_tool.set_up(TEST_INTERFACE, true).unwrap();
    }

    #[test]
    #[ignore]
    fn mtu() {
        let ip_tool = IpTool::new().unwrap();

        ip_tool.set_mtu(TEST_INTERFACE, 1420).unwrap();

        assert_eq!(ip_tool.get_mtu(TEST_INTERFACE).unwrap(), 1420);
    }

    #[test]
    #[ignore]
    fn mac() {
        let ip_tool = IpTool::new().unwrap();
        let mac = "5A:E6:60:8F:5F:DE";

        ip_tool.set_mac(TEST_INTERFACE, mac).unwrap();

        let sa_data = ip_tool.get_mac_sa_data(TEST_INTERFACE).unwrap();
        assert_eq!(sa_data, super::parse_mac_addr(mac).unwrap());
    }

    #[test]
    #[ignore]
    fn set_ipv4() {
        let ip_tool = IpTool::new().unwrap();
        let address: Ipv4Addr = "10.23.42.1".parse().unwrap();

        ip_tool
            .set_address(TEST_INTERFACE, &IpAddr::V4(address), 24)
            .unwrap();
    }
}
