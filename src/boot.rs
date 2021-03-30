use crate::memory::{AddressRange, Physical};

#[derive(Debug)]
#[repr(C, align(16))]
pub struct BootInfo {
    pub used_pages: AddressRange<Physical>,
    pub _fill: usize,
}

impl core::fmt::Display for BootInfo {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Used pages: {}", self.used_pages)
    }
}
