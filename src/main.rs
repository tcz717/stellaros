//! # StellarOS
//! An OS in actor modal

#![no_std]
#![no_main]
#![feature(asm)]
#![feature(global_asm)]
#![feature(const_fn)]
#![feature(const_panic)]
#![feature(const_fn_fn_ptr_basics)]
#![feature(format_args_nl)]

#[macro_use]
mod debug;

mod arch;
mod bsp;
mod common;
mod cpu;
mod memory;
mod panic;
mod runtime_init;

use arch::exception::handling_init;

/// Early init code.
///
/// # Safety
///
/// - Only a single core must be active and running this function.
/// - The init calls in this function must appear in the correct order:
///     - Virtual memory must be activated before the device drivers.
///       - Without it, any atomic operations, e.g. the yet-to-be-introduced spinlocks in the device
///         drivers (which currently employ IRQSafeNullLocks instead of spinlocks), will fail to
///         work on the RPi SoCs.
#[no_mangle]
unsafe fn kernel_init() -> ! {
    handling_init();
    println!("StellarOS started!");
    use cpu::qemu_exit_success;
    qemu_exit_success()
}
