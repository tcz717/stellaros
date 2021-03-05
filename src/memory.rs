// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2018-2021 Andre Richter <andre.o.richter@gmail.com>

//! Memory Management.

// pub mod mmu;

use crate::common;
use core::{marker::PhantomData, ops::RangeInclusive};

pub use crate::bsp::memory::*;

//--------------------------------------------------------------------------------------------------
// Public Definitions
//--------------------------------------------------------------------------------------------------

/// Metadata trait for marking the type of an address.
pub trait AddressType: Copy + Clone + PartialOrd + PartialEq {}

/// Zero-sized type to mark a physical address.
#[derive(Copy, Clone, PartialOrd, PartialEq)]
pub enum Physical {}

/// Zero-sized type to mark a virtual address.
#[derive(Copy, Clone, PartialOrd, PartialEq)]
pub enum Virtual {}

/// Generic address type.
#[derive(Copy, Clone, PartialOrd, PartialEq)]
pub struct Address<ATYPE: AddressType> {
    value: usize,
    _address_type: PhantomData<fn() -> ATYPE>,
}

/// Generic address range type.
#[derive(Copy, Clone)]
pub struct AddressRange<ATYPE: AddressType> {
    addr: Address<ATYPE>,
    size: usize,
}
//--------------------------------------------------------------------------------------------------
// Public Code
//--------------------------------------------------------------------------------------------------

impl AddressType for Physical {}
impl AddressType for Virtual {}

impl<ATYPE: AddressType> Address<ATYPE> {
    /// Create an instance.
    pub const fn new(value: usize) -> Self {
        Self {
            value,
            _address_type: PhantomData,
        }
    }

    /// Align down.
    pub const fn align_down(self, alignment: usize) -> Self {
        let aligned = common::align_down(self.value, alignment);

        Self {
            value: aligned,
            _address_type: PhantomData,
        }
    }

    /// Converts `Address` into an usize.
    pub const fn into_usize(self) -> usize {
        self.value
    }
}

impl<ATYPE: AddressType> core::ops::Add<usize> for Address<ATYPE> {
    type Output = Self;

    fn add(self, other: usize) -> Self {
        Self {
            value: self.value + other,
            _address_type: PhantomData,
        }
    }
}

impl<ATYPE: AddressType> core::ops::Sub<usize> for Address<ATYPE> {
    type Output = Self;

    fn sub(self, other: usize) -> Self {
        Self {
            value: self.value - other,
            _address_type: PhantomData,
        }
    }
}

impl<ATYPE: AddressType> AddressRange<ATYPE> {
    #[inline(always)]
    pub const fn new(addr: Address<ATYPE>, size: usize) -> Self {
        Self { addr, size }
    }
    #[inline(always)]
    pub const fn new_raw(addr: usize, size: usize) -> Self {
        Self {
            addr: Address::new(addr),
            size,
        }
    }
    #[inline(always)]
    pub fn addr(&self) -> Address<ATYPE> {
        self.addr
    }
    #[inline(always)]
    pub fn size(&self) -> usize {
        self.size
    }
    pub fn range<T>(&self) -> RangeInclusive<*mut T> {
        RangeInclusive::new(
            self.addr.value as *mut T,
            (self.addr.value + self.size) as *mut T,
        )
    }
}

/// Zero out an inclusive memory range.
///
/// # Safety
///
/// - `range.start` and `range.end` must be valid.
/// - `range.start` and `range.end` must be `T` aligned.
pub unsafe fn zero_volatile<T>(range: RangeInclusive<*mut T>)
where
    T: From<u8>,
{
    let mut ptr = *range.start();
    let end_inclusive = *range.end();

    while ptr <= end_inclusive {
        core::ptr::write_volatile(ptr, T::from(0));
        ptr = ptr.offset(1);
    }
}

// //--------------------------------------------------------------------------------------------------
// // Testing
// //--------------------------------------------------------------------------------------------------

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use test_macros::kernel_test;

//     /// Check `zero_volatile()`.
//     #[kernel_test]
//     fn zero_volatile_works() {
//         let mut x: [usize; 3] = [10, 11, 12];
//         let x_range = x.as_mut_ptr_range();
//         let x_range_inclusive =
//             RangeInclusive::new(x_range.start, unsafe { x_range.end.offset(-1) });

//         unsafe { zero_volatile(x_range_inclusive) };

//         assert_eq!(x, [0, 0, 0]);
//     }

//     /// Check `bss` section layout.
//     #[kernel_test]
//     fn bss_section_is_sane() {
//         use crate::bsp::memory::bss_range_inclusive;
//         use core::mem;

//         let start = *bss_range_inclusive().start() as usize;
//         let end = *bss_range_inclusive().end() as usize;

//         assert_eq!(start % mem::size_of::<usize>(), 0);
//         assert_eq!(end % mem::size_of::<usize>(), 0);
//         assert!(end >= start);
//     }
// }
