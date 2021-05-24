// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2021 Andre Richter <andre.o.richter@gmail.com>

//! Architectural translation table.
//!
//! Only 64 KiB granule is supported.
//!
//! # Orientation
//!
//! Since arch modules are imported into generic modules using the path attribute, the path of this
//! file is:
//!
//! crate::memory::mmu::translation_table::arch_translation_table

use crate::{
    bsp::config::MmuGranule,
    memory::{
        AccessPermissions, AddrMapper, Address, AddressRange, AttributeFields, MemAttributes,
        PageAllocator, Physical, Virtual,
    },
    mmu::TranslationGranule,
};
use core::{convert, marker::PhantomData, mem::ManuallyDrop};
use cortex_a::regs::{RegisterReadWrite, MAIR_EL1, SCTLR_EL1, TCR_EL1, TTBR0_EL1, TTBR1_EL1};
use register::{mmio::ReadWrite, register_bitfields, InMemoryRegister};
use tock_registers::registers::{Readable, Writeable};

pub type Granule4KiB = TranslationGranule<{ 4 * 1024 }>;
pub type Granule16KiB = TranslationGranule<{ 16 * 1024 }>;
pub type Granule64KiB = TranslationGranule<{ 64 * 1024 }>;

pub const ENTRY_PER_TABLE: usize = MmuGranule::SIZE >> 3;

// /// The min supported address space size.
// pub const MIN_ADDR_SPACE_SIZE: usize = 1024 * 1024 * 1024; // 1 GiB

// /// The max supported address space size.
// pub const MAX_ADDR_SPACE_SIZE: usize = 32 * 1024 * 1024 * 1024; // 32 GiB

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum MmuLevel {
    Level0,
    Level1,
    Level2,
    Level3,
}
impl MmuLevel {
    const fn next_lvl(&self) -> Option<MmuLevel> {
        match self {
            Self::Level0 => Some(Self::Level1),
            Self::Level1 => Some(Self::Level2),
            Self::Level2 => Some(Self::Level3),
            _ => None,
        }
    }
}
pub enum EntryType<'a> {
    Invalid,
    Block(&'a mut ReadWrite<u64, STAGE1_TABLE_DESCRIPTOR::Register>),
    Table(&'a mut ReadWrite<u64, STAGE1_TABLE_DESCRIPTOR::Register>),
    Page(&'a mut ReadWrite<u64, STAGE1_PAGE_DESCRIPTOR::Register>),
}

impl<'a> EntryType<'a> {
    pub fn from_entry(entry: &mut TableDescriptor, level: MmuLevel) -> Option<Self> {
        let vaild = STAGE1_TABLE_DESCRIPTOR::VALID::True.matches_all(entry.value);
        if !vaild {
            return Some(Self::Invalid);
        }
        let is_table = STAGE1_TABLE_DESCRIPTOR::TYPE::Table.matches_all(entry.value);
        unsafe {
            if is_table {
                if level == MmuLevel::Level3 {
                    Some(Self::Page(core::mem::transmute(entry)))
                } else {
                    Some(Self::Table(core::mem::transmute(entry)))
                }
            } else {
                if level == MmuLevel::Level0 {
                    None
                } else {
                    Some(Self::Block(core::mem::transmute(entry)))
                }
            }
        }
    }
}

// A table descriptor, as per ARMv8-A Architecture Reference Manual Figure D5-15.
register_bitfields! {u64,
    STAGE1_TABLE_DESCRIPTOR [
        /// Physical address of the next descriptor.
        NEXT_LEVEL_TABLE_ADDR OFFSET(crate::bsp::config::MmuGranule::SHIFT) NUMBITS(48 - crate::bsp::config::MmuGranule::SHIFT) [], // [47:m]

        TYPE  OFFSET(1) NUMBITS(1) [
            Block = 0,
            Table = 1
        ],

        VALID OFFSET(0) NUMBITS(1) [
            False = 0,
            True = 1
        ]
    ]
}

// A level 3 page descriptor, as per ARMv8-A Architecture Reference Manual Figure D5-17.
register_bitfields! {u64,
    STAGE1_PAGE_DESCRIPTOR [
        /// Unprivileged execute-never.
        UXN      OFFSET(54) NUMBITS(1) [
            False = 0,
            True = 1
        ],

        /// Privileged execute-never.
        PXN      OFFSET(53) NUMBITS(1) [
            False = 0,
            True = 1
        ],

        /// Physical address of the next table descriptor (lvl2) or the page descriptor (lvl3).
        OUTPUT_ADDR OFFSET(crate::bsp::config::MmuGranule::SHIFT) NUMBITS(48 - crate::bsp::config::MmuGranule::SHIFT) [], // [47:m]

        /// Access flag.
        AF       OFFSET(10) NUMBITS(1) [
            False = 0,
            True = 1
        ],

        /// Shareability field.
        SH       OFFSET(8) NUMBITS(2) [
            OuterShareable = 0b10,
            InnerShareable = 0b11
        ],

        /// Access Permissions.
        AP       OFFSET(6) NUMBITS(2) [
            RW_EL1 = 0b00,
            RW_EL1_EL0 = 0b01,
            RO_EL1 = 0b10,
            RO_EL1_EL0 = 0b11
        ],

        /// Memory attributes index into the MAIR_EL1 register.
        AttrIndx OFFSET(2) NUMBITS(3) [],

        TYPE     OFFSET(1) NUMBITS(1) [
            Block = 0,
            Table = 1
        ],

        VALID    OFFSET(0) NUMBITS(1) [
            False = 0,
            True = 1
        ]
    ]
}

/// A table descriptor for 64 KiB aperture.
///
/// The output points to the next table.
#[derive(Copy, Clone)]
#[repr(C)]
pub struct TableDescriptor {
    value: u64,
}

/// A page descriptor with 64 KiB aperture.
///
/// The output points to physical memory.
#[derive(Copy, Clone)]
#[repr(C)]
struct PageDescriptor {
    value: u64,
}

trait BaseAddr {
    fn base_addr_u64(&self) -> u64;
    fn base_addr_usize(&self) -> usize;
}

impl<T, const N: usize> BaseAddr for [T; N] {
    fn base_addr_u64(&self) -> u64 {
        self as *const T as u64
    }

    fn base_addr_usize(&self) -> usize {
        self as *const _ as usize
    }
}

#[repr(transparent)]
pub struct TableSection {
    entries: [TableDescriptor; MmuGranule::SIZE / core::mem::size_of::<TableDescriptor>()],
}

impl TableSection {
    pub fn entry_of_addr(&mut self, vaddr: Address<Virtual>, mask: usize) -> &mut TableDescriptor {
        assert!(
            (mask / (ENTRY_PER_TABLE - 1)).is_power_of_two(),
            "{:#x} is not shifted by {:#x}",
            mask,
            ENTRY_PER_TABLE - 1
        );
        let idx = (vaddr.into_usize() & mask) >> mask.trailing_zeros();
        &mut self.entries[idx]
    }

    pub unsafe fn from_paddr<MAPPER: AddrMapper>(paddr: Address<Physical>) -> &'static mut Self {
        &mut *(MAPPER::map_to_vaddr(paddr).into_usize() as *mut _)
    }
}

impl Default for TableSection {
    fn default() -> Self {
        Self {
            entries: [TableDescriptor { value: 0 };
                MmuGranule::SIZE / core::mem::size_of::<TableDescriptor>()],
        }
    }
}

pub trait MmuReigon<MAPPER: AddrMapper, ALLOC: PageAllocator> {
    fn root(&self) -> Option<&TableSection>;
    fn root_mut(&mut self) -> Option<&mut TableSection>;
    fn root_or_init(&mut self) -> &mut TableSection;

    fn map_range_with(
        &mut self,
        prange: AddressRange<Physical>,
        vrange: AddressRange<Virtual>,
        attribute: AttributeFields,
    ) -> Result<(), &'static str> {
        assert_eq!(prange.size(), vrange.size());
        assert!(
            prange.addr().is_aligned(MmuGranule::SIZE),
            "prange = {} not aligned with {:#x}",
            prange.addr(),
            MmuGranule::SIZE
        );
        assert!(
            vrange.addr().is_aligned(MmuGranule::SIZE),
            "vrange = {} not aligned with {:#x}",
            vrange.addr(),
            MmuGranule::SIZE
        );
        let page_map = prange.pages().zip(vrange.pages());

        for (paddr, vaddr) in page_map {
            self.map_page(paddr, vaddr, attribute)?;
        }
        Ok(())
    }

    fn map_range(
        &mut self,
        range: AddressRange<Physical>,
        attribute: AttributeFields,
    ) -> Result<(), &'static str> {
        let prange = range;
        let vrange = MAPPER::map_to_vrange(prange);
        self.map_range_with(prange, vrange, attribute)
    }

    fn map_page(
        &mut self,
        paddr: Address<Physical>,
        vaddr: Address<Virtual>,
        attributes: AttributeFields,
    ) -> Result<(), &'static str> {
        // println!("*Map {} to {}", paddr, vaddr);
        let mut mask: usize = 0xFF80_0000_0000;
        let mut section = self.root_or_init();
        let mut level = MmuLevel::Level0;
        while mask > MmuGranule::SIZE {
            let entry = section.entry_of_addr(vaddr, mask);
            match EntryType::from_entry(entry, level) {
                Some(EntryType::Block(_)) => return Err("Address already mapped in a block"),
                Some(EntryType::Page(_)) => return Err("Address already mapped in a page"),
                None => return Err("Block descriptor cannot be in level0"),
                Some(EntryType::Table(table)) => {
                    let next_table = (table.read(STAGE1_TABLE_DESCRIPTOR::NEXT_LEVEL_TABLE_ADDR)
                        << MmuGranule::SHIFT) as usize;
                    unsafe {
                        section = TableSection::from_paddr::<MAPPER>(Address::new(next_table));
                        level = level.next_lvl().unwrap();
                    }
                }
                Some(EntryType::Invalid) => {
                    if level == MmuLevel::Level3 {
                        let val = InMemoryRegister::<u64, STAGE1_PAGE_DESCRIPTOR::Register>::new(0);

                        let shifted = paddr.into_usize() as u64 >> MmuGranule::SHIFT;
                        val.write(
                            STAGE1_PAGE_DESCRIPTOR::VALID::True
                                + STAGE1_PAGE_DESCRIPTOR::AF::True
                                + attributes.into()
                                + STAGE1_PAGE_DESCRIPTOR::TYPE::Table
                                + STAGE1_PAGE_DESCRIPTOR::OUTPUT_ADDR.val(shifted),
                        );
                        entry.value = val.get();
                        // println!(
                        //     "Page desc: {:#x} to {:#x}",
                        //     val.get(),
                        //     entry as *const _ as usize
                        // );
                    } else {
                        let next_table = ManuallyDrop::new(ALLOC::alloc_pages(1)?);
                        unsafe { next_table.as_bytes_mut::<MAPPER>().fill(0) }
                        *entry = TableDescriptor::from_next_lvl_table_addr(next_table.base());

                        continue;
                    }
                }
            }
            mask >>= MmuGranule::SHIFT - 3;
        }
        Ok(())
    }
}

/// Wraper for TTBR0_EL1
pub struct MmuReigon0<MAPPER: AddrMapper, ALLOC: PageAllocator> {
    _alloc: PhantomData<ALLOC>,
    _mapper: PhantomData<MAPPER>,
}

impl<MAPPER: AddrMapper, ALLOC: PageAllocator> MmuReigon<MAPPER, ALLOC>
    for MmuReigon0<MAPPER, ALLOC>
{
    fn root(&self) -> Option<&TableSection> {
        let paddr = MAPPER::map_to_vaddr(Address::new(TTBR0_EL1.get_baddr() as usize));
        unsafe { (paddr.into_usize() as *const TableSection).as_ref() }
    }
    fn root_mut(&mut self) -> Option<&mut TableSection> {
        let paddr = MAPPER::map_to_vaddr(Address::new(TTBR0_EL1.get_baddr() as usize));
        unsafe { (paddr.into_usize() as *mut TableSection).as_mut() }
    }
    fn root_or_init(&mut self) -> &mut TableSection {
        self.root_mut().unwrap_or_else(|| {
            let lvl0 = ALLOC::alloc_pages(1).expect("get level0 table space");
            unsafe {
                lvl0.as_bytes_mut::<MAPPER>().fill(0);
                TTBR0_EL1.set_baddr(lvl0.base().into_usize() as u64);
                let (paddr, _) = lvl0.into_raw();
                &mut *(MAPPER::map_to_vaddr(paddr).into_usize() as *mut _)
            }
        })
    }
}

/// Wraper for TTBR1_EL1
pub struct MmuReigon1<MAPPER: AddrMapper, ALLOC: PageAllocator> {
    _alloc: PhantomData<ALLOC>,
    _mapper: PhantomData<MAPPER>,
}

impl<MAPPER: AddrMapper, ALLOC: PageAllocator> MmuReigon<MAPPER, ALLOC>
    for MmuReigon1<MAPPER, ALLOC>
{
    fn root(&self) -> Option<&TableSection> {
        let paddr = MAPPER::map_to_vaddr(Address::new(TTBR1_EL1.get_baddr() as usize));
        unsafe { (paddr.into_usize() as *const TableSection).as_ref() }
    }
    fn root_mut(&mut self) -> Option<&mut TableSection> {
        let paddr = MAPPER::map_to_vaddr(Address::new(TTBR1_EL1.get_baddr() as usize));
        unsafe { (paddr.into_usize() as *mut TableSection).as_mut() }
    }
    fn root_or_init(&mut self) -> &mut TableSection {
        self.root_mut().unwrap_or_else(|| {
            let lvl0 = ALLOC::alloc_pages(1).expect("get level0 table space");
            unsafe {
                lvl0.as_bytes_mut::<MAPPER>().fill(0);
                TTBR1_EL1.set_baddr(lvl0.base().into_usize() as u64);
                let (paddr, _) = lvl0.into_raw();
                &mut *(MAPPER::map_to_vaddr(paddr).into_usize() as *mut _)
            }
        })
    }
}

pub struct MemoryManagementUnit<ALLOC: PageAllocator> {
    _alloc: PhantomData<ALLOC>,
}

impl<ALLOC: PageAllocator> MemoryManagementUnit<ALLOC> {
    pub unsafe fn new() -> Self {
        Self {
            _alloc: PhantomData,
        }
    }
    pub fn ttbl0<MAPPER: AddrMapper>(&mut self) -> &mut MmuReigon0<MAPPER, ALLOC> {
        unsafe { &mut *core::ptr::null_mut() }
    }
    pub fn ttbl1<MAPPER: AddrMapper>(&mut self) -> &mut MmuReigon1<MAPPER, ALLOC> {
        unsafe { &mut *core::ptr::null_mut() }
    }

    /// Setup function for the MAIR_EL1 register.
    fn set_up_mair(&self) {
        // Define the memory types being mapped.
        MAIR_EL1.write(
            // Attribute 1 - Cacheable normal DRAM.
            MAIR_EL1::Attr1_Normal_Outer::WriteBack_NonTransient_ReadWriteAlloc +
            MAIR_EL1::Attr1_Normal_Inner::WriteBack_NonTransient_ReadWriteAlloc +

        // Attribute 0 - Device.
            MAIR_EL1::Attr0_Device::nonGathering_nonReordering_EarlyWriteAck,
        );
    }

    /// Configure various settings of stage 1 of the EL1 translation regime.
    pub fn enable(&mut self) {
        let t0sz = (64 - 40) as u64;
        let t1sz = (64 - 48) as u64;

        self.set_up_mair();

        TCR_EL1.write(
            TCR_EL1::TBI0::Used
                + TCR_EL1::TG0::KiB_4
                + TCR_EL1::SH0::Inner
                + TCR_EL1::TBI1::Used
                + TCR_EL1::TG1::KiB_4
                + TCR_EL1::SH1::Inner
                + TCR_EL1::ORGN0::WriteBack_ReadAlloc_WriteAlloc_Cacheable
                + TCR_EL1::IRGN0::WriteBack_ReadAlloc_WriteAlloc_Cacheable
                + TCR_EL1::ORGN1::WriteBack_ReadAlloc_WriteAlloc_Cacheable
                + TCR_EL1::IRGN1::WriteBack_ReadAlloc_WriteAlloc_Cacheable
                + TCR_EL1::EPD0::EnableTTBR0Walks
                + TCR_EL1::EPD1::EnableTTBR1Walks
                + TCR_EL1::IPS::Bits_40
                + TCR_EL1::A1::TTBR0
                + TCR_EL1::T0SZ.val(t0sz)
                + TCR_EL1::T1SZ.val(t1sz),
        );

        // Enable MMU
        unsafe {
            cortex_a::barrier::isb(cortex_a::barrier::SY);

            SCTLR_EL1.write(SCTLR_EL1::M::Enable);

            cortex_a::barrier::isb(cortex_a::barrier::SY);
        }
    }
}

/// Constants for indexing the MAIR_EL1.
#[allow(dead_code)]
pub mod mair {
    pub const DEVICE: u64 = 0;
    pub const NORMAL: u64 = 1;
}

// const NUM_LVL2_TABLES: usize = KernelAddrSpaceSize::SIZE >> Granule512MiB::SHIFT;

impl TableDescriptor {
    /// Create an instance.
    ///
    /// Descriptor is invalid by default.
    pub const fn new_zeroed() -> Self {
        Self { value: 0 }
    }

    /// Create an instance pointing to the supplied address.
    pub fn from_next_lvl_table_addr(next_lvl_table_addr: Address<Physical>) -> Self {
        let val = InMemoryRegister::<u64, STAGE1_TABLE_DESCRIPTOR::Register>::new(0);

        let shifted = next_lvl_table_addr.into_usize() >> MmuGranule::SHIFT;
        val.write(
            STAGE1_TABLE_DESCRIPTOR::VALID::True
                + STAGE1_TABLE_DESCRIPTOR::TYPE::Table
                + STAGE1_TABLE_DESCRIPTOR::NEXT_LEVEL_TABLE_ADDR.val(shifted as u64),
        );

        TableDescriptor { value: val.get() }
    }
}

// /// Convert the kernel's generic memory attributes to HW-specific attributes of the MMU.
impl convert::From<AttributeFields>
    for register::FieldValue<u64, STAGE1_PAGE_DESCRIPTOR::Register>
{
    fn from(attribute_fields: AttributeFields) -> Self {
        // Memory attributes.
        let mut desc = match attribute_fields.mem_attributes {
            MemAttributes::CacheableDRAM => {
                STAGE1_PAGE_DESCRIPTOR::SH::InnerShareable
                    + STAGE1_PAGE_DESCRIPTOR::AttrIndx.val(mair::NORMAL)
            }
            MemAttributes::Device => {
                STAGE1_PAGE_DESCRIPTOR::SH::OuterShareable
                    + STAGE1_PAGE_DESCRIPTOR::AttrIndx.val(mair::DEVICE)
            }
        };

        // Access Permissions.
        desc += match attribute_fields.acc_perms {
            AccessPermissions::ReadOnly => STAGE1_PAGE_DESCRIPTOR::AP::RO_EL1,
            AccessPermissions::ReadWrite => STAGE1_PAGE_DESCRIPTOR::AP::RW_EL1,
        };

        // The execute-never attribute is mapped to PXN in AArch64.
        desc += if attribute_fields.execute_never {
            STAGE1_PAGE_DESCRIPTOR::PXN::True
        } else {
            STAGE1_PAGE_DESCRIPTOR::PXN::False
        };

        // Always set unprivileged exectue-never as long as userspace is not implemented yet.
        desc += STAGE1_PAGE_DESCRIPTOR::UXN::True;

        desc
    }
}

// impl PageDescriptor {
//     /// Create an instance.
//     ///
//     /// Descriptor is invalid by default.
//     pub const fn new_zeroed() -> Self {
//         Self { value: 0 }
//     }

//     /// Create an instance.
//     pub fn from_output_addr(output_addr: usize, attribute_fields: AttributeFields) -> Self {
//         let val = InMemoryRegister::<u64, STAGE1_PAGE_DESCRIPTOR::Register>::new(0);

//         let shifted = output_addr as u64 >> Granule64KiB::SHIFT;
//         val.write(
//             STAGE1_PAGE_DESCRIPTOR::VALID::True
//                 + STAGE1_PAGE_DESCRIPTOR::AF::True
//                 + attribute_fields.into()
//                 + STAGE1_PAGE_DESCRIPTOR::TYPE::Table
//                 + STAGE1_PAGE_DESCRIPTOR::OUTPUT_ADDR_64KiB.val(shifted),
//         );

//         Self { value: val.get() }
//     }
// }

// //--------------------------------------------------------------------------------------------------
// // Public Code
// //--------------------------------------------------------------------------------------------------

// impl<const NUM_TABLES: usize> FixedSizeTranslationTable<NUM_TABLES> {
//     /// Create an instance.
//     #[allow(clippy::assertions_on_constants)]
//     pub const fn new() -> Self {
//         assert!(NUM_TABLES > 0);
//         assert!((KernelAddrSpaceSize::SIZE % Granule512MiB::SIZE) == 0);

//         Self {
//             lvl3: [[PageDescriptor::new_zeroed(); 8192]; NUM_TABLES],
//             lvl2: [TableDescriptor::new_zeroed(); NUM_TABLES],
//         }
//     }

//     /// Iterates over all static translation table entries and fills them at once.
//     ///
//     /// # Safety
//     ///
//     /// - Modifies a `static mut`. Ensure it only happens from here.
//     pub unsafe fn populate_tt_entries(&mut self) -> Result<(), &'static str> {
//         for (l2_nr, l2_entry) in self.lvl2.iter_mut().enumerate() {
//             *l2_entry =
//                 TableDescriptor::from_next_lvl_table_addr(self.lvl3[l2_nr].base_addr_usize());

//             for (l3_nr, l3_entry) in self.lvl3[l2_nr].iter_mut().enumerate() {
//                 let virt_addr = (l2_nr << Granule512MiB::SHIFT) + (l3_nr << Granule64KiB::SHIFT);

//                 let (output_addr, attribute_fields) =
//                     virt_mem_layout().virt_addr_properties(virt_addr)?;

//                 *l3_entry = PageDescriptor::from_output_addr(output_addr, attribute_fields);
//             }
//         }

//         Ok(())
//     }

//     /// The translation table's base address to be used for programming the MMU.
//     pub fn base_address(&self) -> u64 {
//         self.lvl2.base_addr_u64()
//     }
// }
