use core::cell::UnsafeCell;
use core::ops::RangeInclusive;

use crate::memory::{Address, Physical, Virtual};

// Symbols from the linker script.
extern "Rust" {
    static __bss_start: UnsafeCell<u64>;
    static __bss_end_inclusive: UnsafeCell<u64>;
    static __ro_start: UnsafeCell<()>;
    static __ro_size: UnsafeCell<()>;
    static __data_size: UnsafeCell<()>;
}

/// The board's physical memory map.
pub(super) mod map {
    use super::*;

    pub const BOOT_CORE_STACK_SIZE: usize = 0x1_0000;

    /// Physical devices.
    #[warn(unused_variables)]
    pub mod mmio {
        use crate::memory::AddressRange;

        use super::*;

        // https://github.com/qemu/qemu/blob/master/hw/arm/virt.c
        /* Addresses and sizes of our components.
         * 0..128MB is space for a flash device so we can run bootrom code such as UEFI.
         * 128MB..256MB is used for miscellaneous device I/O.
         * 256MB..1GB is reserved for possible future PCI support (ie where the
         * PCI memory window will go if we add a PCI host controller).
         * 1GB and up is RAM (which may happily spill over into the
         * high memory region beyond 4GB).
         * This represents a compromise between how much RAM can be given to
         * a 32 bit VM and leaving space for expansion and in particular for PCI.
         * Note that devices should generally be placed at multiples of 0x10000,
         * to accommodate guests using 64K pages.
         */
        // static const MemMapEntry base_memmap[] = {
        //     /* Space up to 0x8000000 is reserved for a boot ROM */
        //     [VIRT_FLASH] =              {          0, 0x08000000 },
        //     [VIRT_CPUPERIPHS] =         { 0x08000000, 0x00020000 },
        //     /* GIC distributor and CPU interfaces sit inside the CPU peripheral space */
        //     [VIRT_GIC_DIST] =           { 0x08000000, 0x00010000 },
        //     [VIRT_GIC_CPU] =            { 0x08010000, 0x00010000 },
        //     [VIRT_GIC_V2M] =            { 0x08020000, 0x00001000 },
        //     [VIRT_GIC_HYP] =            { 0x08030000, 0x00010000 },
        //     [VIRT_GIC_VCPU] =           { 0x08040000, 0x00010000 },
        //     /* The space in between here is reserved for GICv3 CPU/vCPU/HYP */
        //     [VIRT_GIC_ITS] =            { 0x08080000, 0x00020000 },
        //     /* This redistributor space allows up to 2*64kB*123 CPUs */
        //     [VIRT_GIC_REDIST] =         { 0x080A0000, 0x00F60000 },
        //     [VIRT_UART] =               { 0x09000000, 0x00001000 },
        //     [VIRT_RTC] =                { 0x09010000, 0x00001000 },
        //     [VIRT_FW_CFG] =             { 0x09020000, 0x00000018 },
        //     [VIRT_GPIO] =               { 0x09030000, 0x00001000 },
        //     [VIRT_SECURE_UART] =        { 0x09040000, 0x00001000 },
        //     [VIRT_SMMU] =               { 0x09050000, 0x00020000 },
        //     [VIRT_PCDIMM_ACPI] =        { 0x09070000, MEMORY_HOTPLUG_IO_LEN },
        //     [VIRT_ACPI_GED] =           { 0x09080000, ACPI_GED_EVT_SEL_LEN },
        //     [VIRT_NVDIMM_ACPI] =        { 0x09090000, NVDIMM_ACPI_IO_LEN},
        //     [VIRT_PVTIME] =             { 0x090a0000, 0x00010000 },
        //     [VIRT_SECURE_GPIO] =        { 0x090b0000, 0x00001000 },
        //     [VIRT_MMIO] =               { 0x0a000000, 0x00000200 },
        //     /* ...repeating for a total of NUM_VIRTIO_TRANSPORTS, each of that size */
        //     [VIRT_PLATFORM_BUS] =       { 0x0c000000, 0x02000000 },
        //     [VIRT_SECURE_MEM] =         { 0x0e000000, 0x01000000 },
        //     [VIRT_PCIE_MMIO] =          { 0x10000000, 0x2eff0000 },
        //     [VIRT_PCIE_PIO] =           { 0x3eff0000, 0x00010000 },
        //     [VIRT_PCIE_ECAM] =          { 0x3f000000, 0x01000000 },
        //     /* Actual RAM size depends on initial RAM and device memory settings */
        //     [VIRT_MEM] =                { GiB, LEGACY_RAMLIMIT_BYTES },
        // };

        pub const FLASH: AddressRange<Physical> = AddressRange::new_raw(0, 0x08000000);
        pub const UART: AddressRange<Physical> = AddressRange::new_raw(0x09000000, 0x00001000);
        pub const GPIO: AddressRange<Physical> = AddressRange::new_raw(0x09030000, 0x00001000);

        pub const END: Address<Physical> = Address::new(0x4001_0000);
    }

    pub const END: Address<Physical> = mmio::END;
}

/// Start address of the Read-Only (RO) range.
///
/// # Safety
///
/// - Value is provided by the linker script and must be trusted as-is.
#[inline(always)]
fn virt_ro_start() -> Address<Virtual> {
    Address::new(unsafe { __ro_start.get() as usize })
}

/// Start address of the boot core's stack.
#[inline(always)]
fn virt_boot_core_stack_start() -> Address<Virtual> {
    virt_ro_start() - map::BOOT_CORE_STACK_SIZE
}

/// Size of the boot core's stack.
#[inline(always)]
fn boot_core_stack_size() -> usize {
    map::BOOT_CORE_STACK_SIZE
}

/// Exclusive end address of the boot core's stack.
#[inline(always)]
pub fn phys_boot_core_stack_end() -> Address<Physical> {
    // The binary is still identity mapped, so we don't need to convert here.
    let end = virt_boot_core_stack_start().into_usize() + boot_core_stack_size();
    Address::new(end)
}

/// Return the inclusive range spanning the .bss section.
///
/// # Safety
///
/// - Values are provided by the linker script and must be trusted as-is.
/// - The linker-provided addresses must be u64 aligned.
pub fn bss_range_inclusive() -> RangeInclusive<*mut u64> {
    let range;
    unsafe {
        range = RangeInclusive::new(__bss_start.get(), __bss_end_inclusive.get());
    }
    assert!(!range.is_empty());

    range
}
