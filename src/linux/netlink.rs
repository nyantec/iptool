use std::borrow::Cow;
use std::io::Error;
use std::io::Result as IoResult;
use std::sync::{Mutex, TryLockError};

use log::warn;
use neli::attr::{AttrHandle, AttrHandleMut};
use neli::consts::nl::{NlTypeWrapper, NlmF, NlmFFlags, Nlmsg};
use neli::consts::rtnl::{Arphrd, IffFlags, Ifla, RtAddrFamily, Rtm};
use neli::consts::socket::NlFamily;
use neli::err::NlError;
use neli::nl::{NlPayload, Nlmsghdr};
use neli::rtnl::{Ifinfomsg, Rtattr};
use neli::socket::NlSocketHandle;
use neli::types::{Buffer, RtBuffer};
use neli::Nl;
use nix::unistd::Pid;

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

    // -- Interfaces --
    pub fn get_interfaces(&self) -> Result<Vec<Interface>, NlError> {
        let mut seq = self
            .seq
            .lock()
            .map_err(|_| Error::from_raw_os_error(libc::ENOTRECOVERABLE))?;

        let mut attrs = RtBuffer::new();
        attrs.push(Rtattr::new(None, Ifla::ExtMask, 0x01000000u32)?);

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
        socket.send(nlhdr);

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

            let payload = match nl.nl_payload {
                NlPayload::Payload(p) => p,
                NlPayload::Err(e) => return Err(e.into()),
                _ => return Err(Error::from_raw_os_error(libc::ENOTRECOVERABLE).into()),
            };

            ret.push(Interface(payload))
        }

        *seq += 1;
        drop(seq);

        Ok(ret)
    }

    pub fn get_interface(&self, dev: &str) -> Result<Interface, NlError> {
        let mut seq = self
            .seq
            .lock()
            .map_err(|_| Error::from_raw_os_error(libc::ENOTRECOVERABLE))?;

        let mut attrs = RtBuffer::new();
        attrs.push(Rtattr::new(None, Ifla::Ifname, dev)?);
        attrs.push(Rtattr::new(None, Ifla::ExtMask, 0x01000000u32)?);

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
        socket.send(nlhdr);

        /*let ret = for nl in socket.iter(false) {
            let nl: Nlmsghdr<NlTypeWrapper, Ifinfomsg> = nl?;


        }*/
        if let Some(ret) = socket.recv()? {
            let ret: Nlmsghdr<NlTypeWrapper, Ifinfomsg> = ret;

            if ret.nl_seq != *seq {
                warn!("Sequence not correct");
                return Err(Error::from_raw_os_error(libc::ENOTRECOVERABLE).into());
            }

            let payload = match ret.nl_payload {
                NlPayload::Payload(p) => p,
                NlPayload::Err(e) => return Err(e.into()),
                _ => return Err(Error::from_raw_os_error(libc::ENOTRECOVERABLE).into()),
            };

            return Ok(Interface(payload));
        }

        unreachable!("should not happen")
    }
}

// -- Interface --
#[derive(Debug)]
pub struct Interface(pub Ifinfomsg);

type AttrHandleInterface<'a> = AttrHandle<'a, RtBuffer<Ifla, Buffer>, Rtattr<Ifla, Buffer>>;
impl Interface {
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
    pub fn get_if_name_handle<'a>(attr_handle: &AttrHandleInterface) -> Result<String, NlError> {
        let name = attr_handle.get_attr_payload_as::<String>(Ifla::Ifname)?;
        Ok(name)
    }
}

#[cfg(test)]
mod test {
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
}
