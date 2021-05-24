//! StellarOS bootloader

#![no_std]
#![no_main]
#![feature(asm)]
#![feature(global_asm)]
#![feature(const_panic)]
#![feature(const_fn_fn_ptr_basics)]
#![feature(format_args_nl)]

use core::{cell::UnsafeCell, mem::ManuallyDrop};

use cortex_a::regs::*;
use debug::UART0;
use elfloader::{ElfBinary, Flags, LoadableHeaders, Rela, VAddr, P64};
use stellaros::{
    arch::{
        mmu::{MemoryManagementUnit, MmuReigon},
        reg::cpacr_el1::CPACR_EL1,
    },
    common::align_up,
    memory::{
        AccessPermissions, Address, AttributeFields, MemAttributes, Page, PageAllocator, Physical,
    },
};
use stellaros::{
    bsp::config::MmuGranule,
    memory::{AddressRange, IdentMapper},
};

#[macro_use]
mod debug;

mod boot;
mod panic;

// Symbols from the linker script.
extern "Rust" {
    static __load_start: UnsafeCell<u8>;
    static __load_end: UnsafeCell<u8>;
}

unsafe fn kernel_elf() -> &'static [u8] {
    include_bytes!("../stellaros")
}

struct KernelLoader {
    mmu: MemoryManagementUnit<StackPageAllocator>,
}

struct StackPageAllocator;

struct StackPageAllocatorMetadata {
    start: Address<Physical>,
    end: Address<Physical>,
    top: Address<Physical>,
}

static mut METADATA: StackPageAllocatorMetadata = StackPageAllocatorMetadata::new();

fn flags_to_attributes(flags: &Flags) -> AttributeFields {
    let ap = if flags.is_write() {
        AccessPermissions::ReadWrite
    } else {
        // TODO: change back to read
        AccessPermissions::ReadWrite
    };
    let nx = !flags.is_execute();
    AttributeFields {
        mem_attributes: MemAttributes::CacheableDRAM,
        acc_perms: ap,
        execute_never: nx,
    }
}

impl elfloader::ElfLoader for KernelLoader {
    fn allocate(&mut self, load_headers: LoadableHeaders) -> Result<(), &'static str> {
        for header in load_headers {
            println!(
                "allocate base = {:#x} size = {:#x} flags = {}",
                header.virtual_addr(),
                header.mem_size(),
                header.flags()
            );
            let aligned_size = align_up(header.mem_size() as usize, MmuGranule::SIZE);
            let pages_num = aligned_size >> MmuGranule::SHIFT;

            let pages = ManuallyDrop::new(StackPageAllocator::alloc_pages(pages_num)?);
            self.mmu.ttbl1::<IdentMapper>().map_range_with(
                pages.range(),
                AddressRange::new_raw(header.virtual_addr() as usize, aligned_size),
                flags_to_attributes(&header.flags()),
            )?;
        }
        Ok(())
    }

    fn relocate(&mut self, _entry: &Rela<P64>) -> Result<(), &'static str> {
        Err("Relocation not supported")
    }

    fn load(&mut self, _flags: Flags, base: VAddr, region: &[u8]) -> Result<(), &'static str> {
        let start = base;
        let end = base + region.len() as u64;
        println!("load region into = {:#x} -- {:#x}", start, end);
        unsafe {
            core::ptr::copy(region.as_ptr(), base as *mut _, region.len());
        }
        Ok(())
    }
}

impl PageAllocator for StackPageAllocator {
    /// TODO: Consider SMP data race
    fn alloc_pages(num: usize) -> Result<Page<Self>, &'static str> {
        let size = num * MmuGranule::SIZE;
        // Only one thread is running at the moment.
        unsafe {
            assert_ne!(METADATA.start.into_usize(), 0);
            // println!(
            //     "Trying to alloc {:#x} size, top {:#x}, end {:#x}",
            //     size,
            //     METADATA.top.into_usize(),
            //     METADATA.end.into_usize()
            // );
            if METADATA.top + size > METADATA.end {
                return Err("Page stack overflow");
            }
            let page = Page::from_raw(METADATA.top, num);
            METADATA.top = METADATA.top + size;
            Ok(page)
        }
    }
    unsafe fn free_pages(_pages: &mut Page<Self>) -> Result<(), &'static str> {
        Err("Page stack free not supported")
    }
}

impl StackPageAllocatorMetadata {
    const fn new() -> Self {
        Self {
            start: Address::new(0),
            end: Address::new(0),
            top: Address::new(0),
        }
    }

    fn init(&mut self, start: Address<Physical>, num: usize) {
        self.start = start;
        self.top = start;
        self.end = start + num * MmuGranule::SIZE;
    }

    const fn range(&self) -> AddressRange<Physical> {
        AddressRange::new_range(self.start, self.end)
    }
}

fn setup_kernel_mmu() -> MemoryManagementUnit<StackPageAllocator> {
    let mut mmu: MemoryManagementUnit<StackPageAllocator> = unsafe { MemoryManagementUnit::new() };

    let range =
        unsafe { AddressRange::new_range(__load_start.get().into(), __load_end.get().into()) };
    let attributes = AttributeFields {
        mem_attributes: MemAttributes::CacheableDRAM,
        acc_perms: AccessPermissions::ReadWrite,
        execute_never: false,
    };
    let ttbl0 = mmu.ttbl0::<IdentMapper>();
    ttbl0
        .map_range(range, attributes)
        .expect("Failed to map image");

    ttbl0
        .map_range(unsafe { METADATA.range() }, attributes)
        .expect("Failed to map page pool");

    ttbl0
        .map_page(
            UART0.into(),
            UART0.into(),
            AttributeFields {
                mem_attributes: MemAttributes::Device,
                acc_perms: AccessPermissions::ReadWrite,
                execute_never: true,
            },
        )
        .expect("Failed to map UART0");

    mmu.enable();

    mmu
}

fn setup_kernel_stack(mmu: &mut MemoryManagementUnit<StackPageAllocator>) -> usize {
    const STACK_PAGES: usize = 512;
    let stack_pages = ManuallyDrop::new(
        StackPageAllocator::alloc_pages(STACK_PAGES).expect("No enough stack size"),
    );
    let stack_vrange = AddressRange::new(
        Address::new(0xFFFF_1000_0000_0000),
        STACK_PAGES * MmuGranule::SIZE,
    );
    mmu.ttbl1::<IdentMapper>()
        .map_range_with(
            stack_pages.range(),
            stack_vrange,
            AttributeFields {
                mem_attributes: MemAttributes::CacheableDRAM,
                acc_perms: AccessPermissions::ReadWrite,
                execute_never: true,
            },
        )
        .expect("Failed to map stack");
    stack_vrange.end().into_usize()
}

fn jump_to_entry(entry_point: usize, stack_end: usize) -> ! {
    println!("Jump to kernel entry");
    unsafe {
        let boot_info = &mut *(stack_end as *mut stellaros::boot::BootInfo).offset(-1);
        boot_info.used_pages = AddressRange::new_range(METADATA.start, METADATA.top);
        let stack_end = boot_info as *const _ as usize;
        asm!(
            "mov SP, x0",
            "br {}",
            in(reg) entry_point,
            in("x0") stack_end,
            options(nomem, nostack, noreturn)
        );
    }
}

#[no_mangle]
unsafe fn main() {
    DAIF.write(DAIF::D::Masked + DAIF::A::Masked + DAIF::F::Masked + DAIF::I::Masked);
    CPACR_EL1.write(CPACR_EL1::FPEN::NONE);

    stellaros::arch::exception::handling_init();
    METADATA.init(
        Address::new(align_up(__load_end.get() as usize, MmuGranule::SIZE)),
        1024,
    );
    let mut mmu = setup_kernel_mmu();

    let stack_end = setup_kernel_stack(&mut mmu);

    let binary = ElfBinary::new("test", kernel_elf()).expect("Got proper ELF section");
    let mut loader = KernelLoader { mmu };
    binary.load(&mut loader).expect("Can't load the binary?");

    jump_to_entry(binary.entry_point() as usize, stack_end)
}
