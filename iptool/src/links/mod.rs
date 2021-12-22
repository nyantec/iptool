use std::io::{Error, Result};

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

    pub fn get_interfaces(&self) -> Result<Vec<Interface>> {
        self.inner.get_interfaces()
    }
}
