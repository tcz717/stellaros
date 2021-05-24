#![no_std]
#![feature(asm)]
#![feature(global_asm)]
#![feature(const_panic)]
#![feature(const_fn_trait_bound)]
#![feature(const_fn_fn_ptr_basics)]
#![feature(format_args_nl)]

#[macro_use]
mod debug;

pub mod arch;
pub mod bsp;
pub mod common;
pub mod cpu;
pub mod memory;
pub mod mmu;
pub mod boot;
mod runtime_init;
