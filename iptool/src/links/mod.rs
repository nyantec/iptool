use std::io::Result;
use std::path::Path;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux as link_impl;

#[doc(inline)]
pub use link_impl::Interface;

pub struct LinkTool {
    inner: link_impl::LinkTool,
}

impl LinkTool {
    pub fn new() -> Result<Self> {
        Ok(Self {
            inner: link_impl::LinkTool::new()?,
        })
    }

    #[cfg(target_os = "linux")]
    // TODO: remove
    pub unsafe fn enter_ns_path<P: AsRef<Path>>(&mut self, path: &P) -> Result<()> {
        self.inner.setns_path(path)
    }

    pub fn create_interface(&mut self, interface: Interface) -> Result<()> {
        self.inner.create_interface(interface)
    }

    pub fn get_interfaces(&self) -> Result<Vec<Interface>> {
        self.inner.get_interfaces()
    }

    pub fn get_interface(&self, name: &str) -> Result<Interface> {
        self.inner.get_interface(name)
    }

    pub fn delete_interface(&mut self, interface: Interface) -> Result<()> {
        self.inner.delete_interface(interface)
    }

    pub unsafe fn get_inner(&self) -> &link_impl::LinkTool {
        &self.inner
    }

    pub unsafe fn get_inner_mut(&mut self) -> &mut link_impl::LinkTool {
        &mut self.inner
    }
}
