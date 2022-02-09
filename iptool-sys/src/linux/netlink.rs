use libc::{c_int, c_uint};
use std::io::Error;
use std::io::Result as IoResult;
use std::os::unix::io::RawFd;
use std::sync::{Mutex, TryLockError};

use log::warn;
use neli::attr::{AttrHandle, AttrHandleMut};
use neli::consts::nl::{NlTypeWrapper, NlmF, NlmFFlags, Nlmsg};
use neli::consts::rtnl::{Arphrd, IffFlags, Ifla, IflaInfo, RtAddrFamily, RtaType, Rtm};
use neli::consts::socket::NlFamily;
use neli::err::{DeError, NlError};
use neli::nl::{NlPayload, Nlmsghdr};
use neli::rtnl::{Ifinfomsg, Rtattr, Rtgenmsg};
use neli::socket::NlSocketHandle;
use neli::types::{Buffer, RtBuffer};
use neli::{FromBytesWithInput, Header};
use nix::unistd::Pid;

macro_rules! get_payload {
    ($payload:expr) => {
        match $payload {
            NlPayload::Payload(p) => p,
            NlPayload::Err(e) => return Err(e.into()),
            _ => return Err(Error::from_raw_os_error(libc::ENOTRECOVERABLE).into()),
        }
    };
}

pub struct RTNetlink {
    handle: std::cell::UnsafeCell<NlSocketHandle>,
    seq: Mutex<u32>,
}

unsafe impl Sync for RTNetlink {}

impl RTNetlink {
    pub fn new() -> IoResult<Self> {
        let handle =
            NlSocketHandle::connect(NlFamily::Route, Some(Pid::this().as_raw() as u32), &[])?;

        handle.block()?;

        if !(handle.is_blocking()?) {
            warn!("handle could not be set into nonblocking")
        }

        Ok(Self {
            handle: handle.into(),
            seq: Mutex::new(0),
        })
    }

    /// Try to get the current sequence number
    ///
    /// # Error
    /// - [`libc::ENOTRECOVERABLE`] - Lock is Poisened, cannot get
    /// - [`libc::EWOULDBLOCK`] - Lock is currently held otherwise
    pub fn get_seq(&self) -> IoResult<u32> {
        //self.seq.lock().map(|seq| *seq).map_err(|e| )
        match self.seq.try_lock() {
            Ok(seq) => Ok(*seq),
            Err(TryLockError::WouldBlock) => Err(Error::from_raw_os_error(libc::EWOULDBLOCK)),
            Err(TryLockError::Poisoned(_)) => Err(Error::from_raw_os_error(libc::ENOTRECOVERABLE)),
        }
    }

    /// Block for the lock on seq to get the current sequence number
    ///
    /// # Error
    /// - [`libc::ENOTRECOVERABLE`] - Lock is Poisened, cannot get
    pub fn get_seq_blocking(&self) -> IoResult<u32> {
        match self.seq.lock() {
            Ok(seq) => Ok(*seq),
            Err(_) => Err(Error::from_raw_os_error(libc::ENOTRECOVERABLE)),
        }
    }

    // -- Network Namspace --
    /// Get the network namespace id for the namespace referred to by `fd`
    pub fn get_nsid_fd(&self, fd: RawFd) -> Result<Option<i32>, NlError<NlTypeWrapper, Netns>> {
        let mut seq = self
            .seq
            .lock()
            .map_err(|_| Error::from_raw_os_error(libc::ENOTRECOVERABLE))?;

        let mut attrs = RtBuffer::new();
        //attrs.push(Rtattr::new(None, Ifla::NetNsFd, fd as u32)?);
        attrs.push(Rtattr::new(None, NetNSA::FD, fd as u32)?);

        let nlhdr = {
            let len = None;
            let nl_type = Rtm::Getnsid;
            let flag = NlmFFlags::new(&[NlmF::Request]);
            let seq = Some(*seq);
            let pid = Some(Pid::this().as_raw() as _);
            let payload = Netns::new(RtAddrFamily::Unspecified, attrs);

            Nlmsghdr::new(len, nl_type, flag, seq, pid, NlPayload::Payload(payload))
        };

        let socket = unsafe { &mut *self.handle.get() };
        socket.send(nlhdr)?;

        let id = if let Some(ret) = socket.recv()? {
            let ret: Nlmsghdr<NlTypeWrapper, Netns> = ret;

            if ret.nl_seq != *seq {
                warn!("Sequence not correct");
                return Err(Error::from_raw_os_error(libc::ENOTRECOVERABLE).into());
            }

            let payload = get_payload!(ret.nl_payload);

            let id = payload
                .rtattrs
                .get_attr_handle()
                .get_attr_payload_as(NetNSA::NSid)?;

            if id < 0 {
                None
            } else {
                Some(id)
            }
        } else {
            return Err(NlError::NoAck);
        };

        Ok(id)
    }

    // -- Interfaces --
    pub fn get_interfaces(
        &self,
        nsid: Option<i32>,
    ) -> Result<Vec<Interface>, NlError<NlTypeWrapper, Ifinfomsg>> {
        let mut seq = self
            .seq
            .lock()
            .map_err(|_| Error::from_raw_os_error(libc::ENOTRECOVERABLE))?;

        let mut attrs = RtBuffer::new();
        attrs.push(Rtattr::new(None, Ifla::ExtMask, 0x01000000u32)?);
        if let Some(nsid) = nsid {
            attrs.push(Rtattr::new(None, Ifla::IfNetnsid, nsid)?);
        }

        let nlhdr = {
            let len = None;
            let nl_type = Rtm::Getlink;
            let flag = NlmFFlags::new(&[NlmF::Request, NlmF::Root, NlmF::Match]);
            let seq = Some(*seq);
            let pid = Some(Pid::this().as_raw() as _);
            let payload = Ifinfomsg::new(
                RtAddrFamily::Unspecified,
                Arphrd::Netrom,
                0,
                IffFlags::new(&[]),
                IffFlags::new(&[]),
                attrs,
            );
            Nlmsghdr::new(len, nl_type, flag, seq, pid, NlPayload::Payload(payload))
        };

        let socket = unsafe { &mut *self.handle.get() };
        socket.send(nlhdr)?;

        let mut ret = Vec::new();

        for nl in socket.iter(false) {
            let nl: Nlmsghdr<NlTypeWrapper, Ifinfomsg> = nl?;

            if nl.nl_seq != *seq {
                warn!("Sequence not correct");
                return Err(Error::from_raw_os_error(libc::ENOTRECOVERABLE).into());
            }

            if let NlTypeWrapper::Nlmsg(Nlmsg::Done) = nl.nl_type {
                break;
            }

            let payload = get_payload!(nl.nl_payload);

            ret.push(Interface(payload))
        }

        *seq += 1;
        drop(seq);

        Ok(ret)
    }

    pub fn get_interface(
        &self,
        dev: &str,
        nsid: Option<i32>,
    ) -> Result<Interface, NlError<NlTypeWrapper, Ifinfomsg>> {
        let mut seq = self
            .seq
            .lock()
            .map_err(|_| Error::from_raw_os_error(libc::ENOTRECOVERABLE))?;

        let mut attrs = RtBuffer::new();
        attrs.push(Rtattr::new(None, Ifla::Ifname, dev)?);
        attrs.push(Rtattr::new(None, Ifla::ExtMask, 0x01000000u32)?);
        if let Some(nsid) = nsid {
            attrs.push(Rtattr::new(None, Ifla::IfNetnsid, nsid)?);
        }

        let nlhdr = {
            let len = None;
            let nl_type = Rtm::Getlink;
            let flag = NlmFFlags::new(&[NlmF::Request]);
            let seq = Some(*seq);
            let pid = Some(Pid::this().as_raw() as _);
            let payload = Ifinfomsg::new(
                RtAddrFamily::Unspecified,
                Arphrd::Netrom,
                0,
                IffFlags::new(&[]),
                IffFlags::new(&[]),
                attrs,
            );
            Nlmsghdr::new(len, nl_type, flag, seq, pid, NlPayload::Payload(payload))
        };

        let socket = unsafe { &mut *self.handle.get() };
        socket.send(nlhdr)?;

        if let Some(ret) = socket.recv()? {
            let ret: Nlmsghdr<NlTypeWrapper, Ifinfomsg> = ret;

            if ret.nl_seq != *seq {
                warn!("Sequence not correct");
                return Err(Error::from_raw_os_error(libc::ENOTRECOVERABLE).into());
            }

            let payload = get_payload!(ret.nl_payload);

            return Ok(Interface(payload));
        }

        return Err(NlError::NoAck);
    }

    pub fn create_interface(
        &self,
        interface: Interface,
    ) -> Result<(), NlError<NlTypeWrapper, Ifinfomsg>> {
        let mut seq = self
            .seq
            .lock()
            .map_err(|_| Error::from_raw_os_error(libc::ENOTRECOVERABLE))?;

        let nlhdr = {
            let len = None;
            let nl_type = Rtm::Newlink;
            let flag = NlmFFlags::new(&[NlmF::Request, NlmF::Ack, NlmF::Match, NlmF::Atomic]);
            let seq = Some(*seq);
            let pid = Some(Pid::this().as_raw() as _);
            Nlmsghdr::new(
                len,
                nl_type,
                flag,
                seq,
                pid,
                NlPayload::Payload(interface.0),
            )
        };

        let socket = unsafe { &mut *self.handle.get() };
        socket.send(nlhdr)?;

        if let Some(ret) = socket.recv()? {
            let ret: Nlmsghdr<NlTypeWrapper, Ifinfomsg> = ret;

            if let NlPayload::Ack(_) = ret.nl_payload {
                if ret.nl_seq != *seq {
                    return Err(NlError::BadSeq);
                }
            } else {
                return Err(NlError::NoAck);
            }
            todo!()
        } else {
            return Err(NlError::NoAck);
        }

        *seq += 1;

        Ok(())
    }

    pub fn set_interface_ns_fd(
        &self,
        dev: c_uint,
        fd: RawFd,
    ) -> Result<(), NlError<NlTypeWrapper, Ifinfomsg>> {
        let mut seq = self
            .seq
            .lock()
            .map_err(|_| Error::from_raw_os_error(libc::ENOTRECOVERABLE))?;

        let mut attrs = RtBuffer::new();
        attrs.push(Rtattr::new(None, Ifla::NetNsFd, fd)?);

        let nlhdr = {
            let len = None;
            let nl_type = Rtm::Newlink;
            let flag = NlmFFlags::new(&[NlmF::Request, NlmF::Ack]);
            let seq = Some(*seq);
            let pid = Some(Pid::this().as_raw() as _);
            let payload = Ifinfomsg::new(
                RtAddrFamily::Unspecified,
                Arphrd::Netrom,
                dev as c_int,
                IffFlags::new(&[]),
                IffFlags::new(&[]),
                attrs,
            );
            Nlmsghdr::new(len, nl_type, flag, seq, pid, NlPayload::Payload(payload))
        };

        *seq += 1;

        let socket = unsafe { &mut *self.handle.get() };
        socket.send(nlhdr)?;

        if let Some(ret) = socket.recv()? {
            let ret: Nlmsghdr<NlTypeWrapper, Ifinfomsg> = ret;

            if let NlPayload::Ack(_) = ret.nl_payload {
                if ret.nl_seq != *seq - 1 {
                    return Err(NlError::BadSeq);
                }
            } else {
                return Err(NlError::NoAck);
            }
        } else {
            return Err(NlError::NoAck);
        }

        Ok(())
    }

    pub fn delete_interface(
        &self,
        interface: Interface,
    ) -> Result<(), NlError<NlTypeWrapper, Ifinfomsg>> {
        let mut seq = self
            .seq
            .lock()
            .map_err(|_| Error::from_raw_os_error(libc::ENOTRECOVERABLE))?;

        let nlhdr = {
            let len = None;
            let nl_type = Rtm::Dellink;
            let flag = NlmFFlags::new(&[NlmF::Request, NlmF::Ack, NlmF::Match, NlmF::Atomic]);
            let seq = Some(*seq);
            let pid = Some(Pid::this().as_raw() as _);
            Nlmsghdr::new(
                len,
                nl_type,
                flag,
                seq,
                pid,
                NlPayload::Payload(interface.0),
            )
        };

        let socket = unsafe { &mut *self.handle.get() };
        socket.send(nlhdr)?;

        if let Some(ret) = socket.recv()? {
            let ret: Nlmsghdr<NlTypeWrapper, Ifinfomsg> = ret;

            if let NlPayload::Ack(_) = ret.nl_payload {
                if ret.nl_seq != *seq {
                    return Err(NlError::BadSeq);
                }
            } else {
                return Err(NlError::NoAck);
            }
        } else {
            return Err(NlError::NoAck);
        }

        *seq += 1;

        Ok(())
    }

    pub fn create_nsid(&self, ns: Netns) -> Result<(), NlError<NlTypeWrapper, Netns>> {
        let mut seq = self
            .seq
            .lock()
            .map_err(|_| Error::from_raw_os_error(libc::ENOTRECOVERABLE))?;

        let nlhdr = {
            let len = None;
            let nl_type = Rtm::Newnsid;
            let flag = NlmFFlags::new(&[NlmF::Ack]); //&[NlmF::Request, NlmF::Ack, NlmF::Atomic]);
            let seq = Some(*seq);
            let pid = Some(Pid::this().as_raw() as _);
            Nlmsghdr::new(len, nl_type, flag, seq, pid, NlPayload::Payload(ns))
        };

        let socket = unsafe { &mut *self.handle.get() };
        socket.send(nlhdr)?;

        if let Some(ret) = socket.recv()? {
            let ret: Nlmsghdr<NlTypeWrapper, Netns> = ret;

            if let NlPayload::Ack(_) = ret.nl_payload {
                if ret.nl_seq != *seq {
                    return Err(NlError::BadSeq);
                }
            } else {
                return Err(NlError::NoAck);
            }
        } else {
            return Err(NlError::NoAck);
        }

        *seq += 1;

        Ok(())
    }
}

// -- NetNS --
use neli_proc_macros::{
    FromBytesWithInput as FromBytesWithInputGen, Header as HeaderGen, Size, ToBytes,
};

// TODO: wrong padding? message cannot be decoded again??
#[derive(Debug, Size, ToBytes, Header, FromBytesWithInput)]
pub struct Netns {
    //pub rtgen_header: Rtgenmsg,
    pub rtgen_family: RtAddrFamily,
    padding: u16,

    padding2: u8,
    #[neli(input = "input.checked_sub(Self::header_size()).ok_or(DeError::UnexpectedEOB)?")]
    pub rtattrs: RtBuffer<NetNSA, Buffer>,
}

impl Netns {
    pub fn new(rtgen_family: RtAddrFamily, rtattrs: RtBuffer<NetNSA, Buffer>) -> Self {
        Self {
            //rtgen_header: Rtgenmsg { rtgen_family },
            rtgen_family,
            rtattrs,
            padding: 0,
            padding2: 0,
        }
    }

    pub fn new_with_id(id: i32) -> Result<Self, NlError> {
        let mut attrs = RtBuffer::new();
        attrs.push(Rtattr::new(None, NetNSA::NSid, id)?);
        Ok(Self::new(RtAddrFamily::Netlink, attrs))
    }

    pub fn set_pid(&mut self, pid: u32) -> Result<(), NlError> {
        self.rtattrs.push(Rtattr::new(None, NetNSA::Pid, pid)?);
        Ok(())
    }

    pub fn get_pid(&self) -> Result<u32, DeError> {
        let handle = self.rtattrs.get_attr_handle();
        handle.get_attr_payload_as(NetNSA::Pid)
    }

    // std::os::unix::io::RawFd is signed, NETNSA_FD is NLA_U32??
    pub fn set_fd(&mut self, fd: u32) -> Result<(), NlError> {
        self.rtattrs.push(Rtattr::new(None, NetNSA::FD, fd)?);
        Ok(())
    }

    pub fn get_fd(&self) -> Result<u32, DeError> {
        let handle = self.rtattrs.get_attr_handle();
        handle.get_attr_payload_as(NetNSA::FD)
    }
}

#[neli::neli_enum(serialized_type = "libc::c_ushort")]
pub enum NetNSA {
    None = 0,
    NSid = 1,
    Pid = 2,
    FD = 3,
    TragetSid = 4,
    CurrentSid = 5,
}

impl RtaType for NetNSA {}

// -- Interface --
#[derive(Debug)]
pub struct Interface(pub Ifinfomsg);

type AttrHandleInterface<'a> = AttrHandle<'a, RtBuffer<Ifla, Buffer>, Rtattr<Ifla, Buffer>>;
impl Interface {
    /// Create new interface from with type
    pub fn new_with_type(kind: &str) -> Result<Self, NlError> {
        let mut attrs = RtBuffer::new();
        let mut linkinfo = Rtattr::new(None, Ifla::Linkinfo, Vec::<u8>::new())?;
        linkinfo.add_nested_attribute(&Rtattr::new(None, IflaInfo::Kind, kind)?)?;
        attrs.push(linkinfo);

        let msg = Ifinfomsg::new(
            RtAddrFamily::Unspecified,
            Arphrd::Netrom,
            0,
            IffFlags::new(&[]),
            IffFlags::new(&[]),
            attrs,
        );

        Ok(Self(msg))
    }

    pub fn get_attr_handle(&self) -> AttrHandleInterface {
        self.0.rtattrs.get_attr_handle()
    }

    pub fn get_attr_handle_mut(
        &mut self,
    ) -> AttrHandleMut<'_, RtBuffer<Ifla, Buffer>, Rtattr<Ifla, Buffer>> {
        self.0.rtattrs.get_attr_handle_mut()
    }

    pub fn get_if_name(&self) -> Result<String, NlError> {
        let attrs = self.get_attr_handle();
        Self::get_if_name_handle(&attrs)
    }

    // static helpers
    pub fn get_if_name_handle(attr_handle: &AttrHandleInterface) -> Result<String, NlError> {
        let name = attr_handle.get_attr_payload_as_with_len::<String>(Ifla::Ifname)?;
        Ok(name)
    }
}

#[cfg(test)]
mod test {
    use super::Interface;
    use super::Netns;
    use super::RTNetlink;

    lazy_static::lazy_static! {
        static ref HANDLE: RTNetlink = RTNetlink::new().unwrap();
    }

    #[test]
    fn get_interfaces() {
        let interfaces = HANDLE.get_interfaces().unwrap();
        assert!(interfaces.len() > 0);

        let lo = &interfaces[0];
        assert_eq!("lo", lo.get_if_name().unwrap());
        println!("{:?}", lo.get_if_name());
    }

    #[test]
    fn get_interface_lo() {
        let interface = HANDLE.get_interface("lo").unwrap();
        assert_eq!("lo", interface.get_if_name().unwrap());
    }

    #[test]
    #[ignore]
    fn create_dummy() {
        HANDLE
            .create_interface(Interface::new_with_type("dummy").unwrap())
            .unwrap();
    }

    #[test]
    #[ignore]
    fn create_veth() {
        HANDLE
            .create_interface(Interface::new_with_type("veth").unwrap())
            .unwrap();
    }

    #[test]
    #[ignore]
    fn create_netns() {
        let mut netns = Netns::new_with_id(5).unwrap();
        netns
            .set_pid(nix::unistd::Pid::this().as_raw() as _)
            .unwrap();

        HANDLE.create_nsid(netns).unwrap();
    }
}
