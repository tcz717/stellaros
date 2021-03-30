// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2018-2021 Andre Richter <andre.o.richter@gmail.com>

//! Memory Management.

// pub mod mmu;

use crate::common;
use crate::{bsp::config::MmuGranule, common::is_aligned};
use core::{marker::PhantomData, ops::RangeInclusive};

pub use crate::bsp::memory::*;

pub trait AddrMapper {
    fn map_to_vaddr(paddr: Address<Physical>) -> Address<Virtual>;
    fn map_to_vrange(prange: AddressRange<Physical>) -> AddressRange<Virtual> {
        AddressRange::new(Self::map_to_vaddr(prange.addr), prange.size)
    }
}

pub struct IdentMapper;

impl AddrMapper for IdentMapper {
    fn map_to_vaddr(paddr: Address<Physical>) -> Address<Virtual> {
        Address::new(paddr.into_usize())
    }
}

/// Metadata trait for marking the type of an address.
pub trait AddressType: Copy + Clone + PartialOrd + PartialEq {}

/// Zero-sized type to mark a physical address.
#[derive(Copy, Clone, PartialOrd, PartialEq, Debug)]
pub enum Physical {}

/// Zero-sized type to mark a virtual address.
#[derive(Copy, Clone, PartialOrd, PartialEq, Debug)]
pub enum Virtual {}

/// Generic address type.
#[derive(Copy, Clone, PartialOrd, PartialEq, Debug)]
pub struct Address<ATYPE: AddressType> {
    value: usize,
    _address_type: PhantomData<fn() -> ATYPE>,
}

/// Generic address range type.
#[derive(Copy, Clone, Debug)]
pub struct AddressRange<ATYPE: AddressType> {
    addr: Address<ATYPE>,
    size: usize,
}

/// Architecture agnostic memory attributes.
#[allow(missing_docs)]
#[derive(Copy, Clone, PartialOrd, PartialEq)]
pub enum MemAttributes {
    CacheableDRAM,
    Device,
}

/// Architecture agnostic access permissions.
#[allow(missing_docs)]
#[derive(Copy, Clone)]
pub enum AccessPermissions {
    ReadOnly,
    ReadWrite,
}

/// Collection of memory attributes.
#[allow(missing_docs)]
#[derive(Copy, Clone)]
pub struct AttributeFields {
    pub mem_attributes: MemAttributes,
    pub acc_perms: AccessPermissions,
    pub execute_never: bool,
}

pub trait PageAllocator {
    fn alloc_pages(num: usize) -> Result<Page<Self>, &'static str>;
    unsafe fn free_pages(pages: &mut Page<Self>) -> Result<(), &'static str>;
}

pub struct Page<ALLOC: PageAllocator + ?Sized> {
    base: Address<Physical>,
    num: usize,
    allocator: PhantomData<ALLOC>,
}

impl<ALLOC: PageAllocator + ?Sized> Page<ALLOC> {
    pub unsafe fn from_raw(base: Address<Physical>, num: usize) -> Self {
        assert!(
            base.value.trailing_zeros() as usize >= MmuGranule::SHIFT,
            "Page not aligned"
        );
        Self {
            base,
            num,
            allocator: PhantomData,
        }
    }

    #[inline(always)]
    pub const fn base(&self) -> Address<Physical> {
        self.base
    }

    #[inline(always)]
    pub const fn size(&self) -> usize {
        self.num * MmuGranule::SIZE
    }

    #[inline(always)]
    pub const fn page_num(&self) -> usize {
        self.num
    }

    #[inline(always)]
    pub const fn range(&self) -> AddressRange<Physical> {
        AddressRange::new_raw(self.base.value, self.size())
    }

    pub fn into_raw(self) -> (Address<Physical>, usize) {
        let raw = (self.base, self.num);
        core::mem::forget(self);
        raw
    }

    pub unsafe fn ref_as<MAPPER: AddrMapper, T>(&self) -> &T {
        assert!(core::mem::size_of::<T>() <= self.size());
        let vaddr = MAPPER::map_to_vaddr(self.base);
        &*(vaddr.into_usize() as *const T)
    }

    pub unsafe fn ref_as_mut<MAPPER: AddrMapper, T>(&mut self) -> &mut T {
        assert!(core::mem::size_of::<T>() <= self.size());
        let vaddr = MAPPER::map_to_vaddr(self.base);
        &mut *(vaddr.into_usize() as *mut T)
    }

    pub unsafe fn as_bytes<MAPPER: AddrMapper>(&self) -> &[u8] {
        let vaddr = MAPPER::map_to_vaddr(self.base);
        &*core::ptr::slice_from_raw_parts(vaddr.into_usize() as *const u8, self.size())
    }

    pub unsafe fn as_bytes_mut<MAPPER: AddrMapper>(&self) -> &mut [u8] {
        let vaddr = MAPPER::map_to_vaddr(self.base);
        &mut *core::ptr::slice_from_raw_parts_mut(vaddr.into_usize() as *mut u8, self.size())
    }
}

impl<ALLOC: PageAllocator + ?Sized> Drop for Page<ALLOC> {
    fn drop(&mut self) {
        unsafe { ALLOC::free_pages(self).expect("Failed to drop pages") }
    }
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

    /// Align down.
    pub const fn align_up(self, alignment: usize) -> Self {
        let aligned = common::align_up(self.value, alignment);

        Self {
            value: aligned,
            _address_type: PhantomData,
        }
    }

    pub const fn is_aligned(&self, alignment: usize) -> bool {
        is_aligned(self.value, alignment)
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

impl<ATYPE: AddressType> core::fmt::Display for Address<ATYPE> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:#x}", self.value)
    }
}

impl<T, ATYPE: AddressType> core::convert::From<*const T> for Address<ATYPE> {
    fn from(cell: *const T) -> Self {
        Self::new(cell as usize)
    }
}

impl<T, ATYPE: AddressType> core::convert::From<*mut T> for Address<ATYPE> {
    fn from(cell: *mut T) -> Self {
        Self::new(cell as usize)
    }
}

impl<ATYPE: AddressType> AddressRange<ATYPE> {
    #[inline(always)]
    pub const fn new(addr: Address<ATYPE>, size: usize) -> Self {
        Self { addr, size }
    }
    #[inline(always)]
    pub const fn new_range(start: Address<ATYPE>, end: Address<ATYPE>) -> Self {
        assert!(start.value <= end.value);
        Self {
            addr: start,
            size: end.value - start.value,
        }
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
    #[inline(always)]
    pub fn end(&self) -> Address<ATYPE> {
        self.addr + self.size
    }
    pub fn range<T>(&self) -> RangeInclusive<*mut T> {
        RangeInclusive::new(
            self.addr.value as *mut T,
            (self.addr.value + self.size) as *mut T,
        )
    }

    pub fn pages(&self) -> impl Iterator<Item = Address<ATYPE>> {
        let base = self.addr.into_usize();
        (base..base + self.size)
            .step_by(MmuGranule::SIZE)
            .map(Address::new)
    }
}

impl<ATYPE: AddressType> core::fmt::Display for AddressRange<ATYPE> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{} + {:#x}", self.addr, self.size())
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
