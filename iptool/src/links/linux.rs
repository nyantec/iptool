use libc::{c_uint, wchar_t};
use nix::fcntl::{self, OFlag};
use std::borrow::Cow;
use std::io::{Error, ErrorKind, Result};
use std::os::unix::io::RawFd;
use std::path::{Path, PathBuf};

use iptool_sys::neli::consts::rtnl::{Arphrd, Iff, IffFlags, Ifla};
use iptool_sys::neli::err::NlError;
use iptool_sys::RTNetlink;

pub struct LinkTool {
    sys: RTNetlink,
    netns_path: Option<PathBuf>,
}

impl LinkTool {
    pub fn new() -> Result<Self> {
        Ok(Self {
            sys: RTNetlink::new()?,
            netns_path: None,
        })
    }

    pub fn create_interface(&mut self, interface: Interface) -> Result<()> {
        self.sys
            .create_interface(interface.interface)
            .map_err(nl_error_to_io)
    }

    pub fn get_interfaces(&self) -> Result<Vec<Interface>> {
        self.get_interfaces_ns(None)
    }
    pub fn get_interfaces_ns(&self, nsid: Option<i32>) -> Result<Vec<Interface>> {
        let interfaces = self
            .sys
            .get_interfaces(nsid)
            .map_err(|e| nl_error_to_io(e))?;
        let mut result = Vec::new();

        for interface in interfaces {
            result.push(Interface::try_from(interface)?)
        }

        Ok(result)
    }

    pub fn get_interface(&self, name: &str) -> Result<Interface> {
        self.get_interface_ns(name, None)
    }
    pub fn get_interface_ns(&self, name: &str, nsid: Option<i32>) -> Result<Interface> {
        self.sys
            .get_interface(name, nsid)
            .map_err(nl_error_to_io)
            .map(Interface::try_from)?
    }

    pub fn set_interface_ns_path<P: AsRef<Path>>(&mut self, dev: &str, path: &P) -> Result<()> {
        let path = path.as_ref();
        let file = path.to_str().unwrap();

        use nix::sched;
        let ns_file = fcntl::open(
            file,
            OFlag::O_RDONLY | OFlag::O_CLOEXEC,
            nix::sys::stat::Mode::empty(),
        )?;

        let ifidx = nix::net::if_::if_nametoindex(dev)?;

        self.sys
            .set_interface_ns_fd(ifidx, ns_file)
            .map_err(nl_error_to_io)?;

        nix::unistd::close(ns_file)?;
        Ok(())
    }

    pub fn setns_path<P: AsRef<Path>>(&mut self, path: &P) -> Result<()> {
        let path = path.as_ref();
        let file = path.to_str().unwrap();

        use nix::sched;
        let ns_file = fcntl::open(
            file,
            OFlag::O_RDONLY | OFlag::O_CLOEXEC,
            nix::sys::stat::Mode::empty(),
        )?;

        sched::setns(ns_file, sched::CloneFlags::CLONE_NEWNET)?;

        nix::unistd::close(ns_file)?;

        sched::unshare(sched::CloneFlags::CLONE_NEWNS)?;

        self.netns_path = Some(path.to_path_buf());

        Ok(())
    }

    pub fn get_nsid_fd(&self, fd: RawFd) -> Result<Option<i32>> {
        self.sys.get_nsid_fd(fd).map_err(nl_error_to_io)
    }

    pub fn get_nsid_path<P: AsRef<Path>>(&self, path: &P) -> Result<Option<i32>> {
        let ns_file = fcntl::open(
            path.as_ref(),
            OFlag::O_RDONLY | OFlag::O_CLOEXEC,
            nix::sys::stat::Mode::empty(),
        )?;

        let id = self.get_nsid_fd(ns_file)?;

        nix::unistd::close(ns_file)?;
        Ok(id)
    }

    pub fn delete_interface(&self, interface: Interface) -> Result<()> {
        self.sys
            .delete_interface(interface.interface)
            .map_err(nl_error_to_io)
    }
}

#[derive(Debug)]
pub struct Interface {
    pub interface: iptool_sys::Interface,
}

macro_rules! gen_set_if_linux {
    ($func:ident, $inner_func:ident, $arg:ident, $type:ident) => {
        #[cfg(target_os = "linux")]
        pub fn $func(&mut self, $arg: $type) -> Result<()> {
            self.interface.$inner_func($arg).map_err(nl_error_to_io)
        }
    };
}

impl Interface {
    pub fn new(if_type: &str) -> Result<Self> {
        let interface = iptool_sys::Interface::new_with_type(if_type).map_err(nl_error_to_io)?;
        Ok(Self { interface })
    }

    pub fn get_name(&self) -> Result<String> {
        self.interface.get_if_name().map_err(|e| nl_error_to_io(e))
    }

    gen_set_if_linux!(set_name, set_if_name, name, String);
    gen_set_if_linux!(set_mtu, set_if_mtu, mtu, u32);
    gen_set_if_linux!(set_txqlen, set_if_txqlen, txqlen, u32);

    pub fn print_info(&self) -> Result<String> {
        let index = self.interface.0.ifi_index;
        let name = self.get_name()?;

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
            addresses.push(format!("link-netns {}", netns));
        }

        let flags = self.flags_str();

        Ok(format!(
            "{index}: {name}: <{flags}> {attrs}\n    link/{link_type} {address}",
            index = index,
            name = name,
            attrs = attrs.join(" "),
            link_type = link_type,
            address = addresses.join(" "),
            flags = flags
        ))
    }

    const PRINTABLE_FLAGS: &'static [Iff] = &[
        Iff::Loopback,
        Iff::Broadcast,
        Iff::Pointopoint,
        Iff::Multicast,
        Iff::Noarp,
        Iff::Allmulti,
        Iff::Promisc,
        Iff::Master,
        Iff::Slave,
        Iff::Debug,
        Iff::Dynamic,
        Iff::Automedia,
        Iff::Portsel,
        Iff::Notrailers,
        Iff::Up,
        Iff::LowerUp,
        Iff::Dormant,
        Iff::Echo,
    ];

    fn flags_str(&self) -> String {
        let mut ret = Vec::new();

        for flag in Self::PRINTABLE_FLAGS {
            if self.interface.0.ifi_flags.contains(flag) {
                ret.push(Self::print_flag_name(flag))
            }
        }

        // TODO: add M-DOWN flag

        ret.join(",")
    }

    const fn print_flag_name(flag: &Iff) -> &'static str {
        match flag {
            Iff::Loopback => "LOOPBACK",
            Iff::Broadcast => "BROADCAST",
            Iff::Pointopoint => "POINTOPOINT",
            Iff::Multicast => "MULTICAST",
            Iff::Noarp => "NOARP",
            Iff::Allmulti => "ALLMULTI",
            Iff::Promisc => "PROMISC",
            Iff::Master => "MASTER",
            Iff::Slave => "SLAVE",
            Iff::Debug => "DEBUG",
            Iff::Dynamic => "DYNAMIC",
            Iff::Automedia => "AUTOMEDIA",
            Iff::Portsel => "PORTSEL",
            Iff::Notrailers => "NOTRAILERS",
            Iff::Up => "UP",
            Iff::LowerUp => "LOWER_UP",
            Iff::Dormant => "DORMANT",
            Iff::Echo => "ECHO",
            _ => unreachable!(),
        }
    }
}

impl TryFrom<iptool_sys::Interface> for Interface {
    type Error = Error;

    fn try_from(interface: iptool_sys::Interface) -> std::result::Result<Self, Self::Error> {
        //let name = interface.get_if_name().map_err(|e| nl_error_to_io(e))?;

        Ok(Self { interface })
    }
}

fn nl_error_to_io<T, P>(error: NlError<T, P>) -> Error {
    match error {
        NlError::Nlmsgerr(e) => Error::from_raw_os_error(-e.error),
        NlError::De(e) => {
            eprint!("could not translate error: {}", e);
            Error::from(ErrorKind::UnexpectedEof)
        }
        _e => {
            // FIXME: logger
            Error::from(ErrorKind::Other)
        }
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
        6 => "UP",
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
