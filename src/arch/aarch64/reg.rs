pub use cortex_a::regs::*;

// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Copyright (c) 2018-2021 by the author(s)
//
// Author(s):
//   - Andre Richter <andre.o.richter@gmail.com>

macro_rules! __read_raw {
    ($width:ty, $asm_instr:tt, $asm_reg_name:tt, $asm_width:tt) => {
        /// Reads the raw bits of the CPU register.
        #[inline]
        fn get(&self) -> $width {
            match () {
                #[cfg(target_arch = "aarch64")]
                () => {
                    let reg;
                    unsafe {
                        asm!(concat!($asm_instr, " {reg:", $asm_width, "}, ", $asm_reg_name), reg = out(reg) reg, options(nomem, nostack));
                    }
                    reg
                }

                #[cfg(not(target_arch = "aarch64"))]
                () => unimplemented!(),
            }
        }
    };
}

macro_rules! __write_raw {
    ($width:ty, $asm_instr:tt, $asm_reg_name:tt, $asm_width:tt) => {
        /// Writes raw bits to the CPU register.
        #[cfg_attr(not(target_arch = "aarch64"), allow(unused_variables))]
        #[inline]
        fn set(&self, value: $width) {
            match () {
                #[cfg(target_arch = "aarch64")]
                () => {
                    unsafe {
                        asm!(concat!($asm_instr, " ", $asm_reg_name, ", {reg:", $asm_width, "}"), reg = in(reg) value, options(nomem, nostack))
                    }
                }

                #[cfg(not(target_arch = "aarch64"))]
                () => unimplemented!(),
            }
        }
    };
}

/// Raw read from system coprocessor registers.
macro_rules! sys_coproc_read_raw {
    ($width:ty, $asm_reg_name:tt, $asm_width:tt) => {
        __read_raw!($width, "mrs", $asm_reg_name, $asm_width);
    };
}

/// Raw write to system coprocessor registers.
macro_rules! sys_coproc_write_raw {
    ($width:ty, $asm_reg_name:tt, $asm_width:tt) => {
        __write_raw!($width, "msr", $asm_reg_name, $asm_width);
    };
}

/// Raw read from (ordinary) registers.
macro_rules! read_raw {
    ($width:ty, $asm_reg_name:tt, $asm_width:tt) => {
        __read_raw!($width, "mov", $asm_reg_name, $asm_width);
    };
}
/// Raw write to (ordinary) registers.
macro_rules! write_raw {
    ($width:ty, $asm_reg_name:tt, $asm_width:tt) => {
        __write_raw!($width, "mov", $asm_reg_name, $asm_width);
    };
}



pub mod cpacr_el1 {
    use register::{cpu::RegisterReadWrite, register_bitfields};

    register_bitfields! {u32,
        pub CPACR_EL1 [
            // Traps EL0 and EL1 System register accesses to all implemented trace registers to EL1, from both
            // Execution states.
            // 0    EL0 and EL1 System register accesses to all implemented trace registers are not trapped
            //      to EL1.
            // 1    EL0 and EL1 System register accesses to all implemented trace registers are trapped to
            //      EL1.
            TTA OFFSET(28) NUMBITS(1) [
                Unmasked = 0,
                Masked = 1
            ],
            // Traps EL0 and EL1 accesses to the SIMD and floating-point registers to EL1, from both Execution
            // states.
            // 00   Causes any instructions in EL0 or EL1 that use the registers associated with
            //      floating-point and Advanced SIMD execution to be trapped.
            // 01   Causes any instructions in EL0 that use the registers associated with floating-point and
            //      Advanced SIMD execution to be trapped, but does not cause any instruction in EL1 to
            //      be trapped.
            // 10   Causes any instructions in EL0 or EL1 that use the registers associated with
            //      floating-point and Advanced SIMD execution to be trapped.
            // 11   Does not cause any instruction to be trapped.
            //      Writes to MVFR0, MVFR1 and MVFR2 from EL1 or higher
            FPEN OFFSET(20) NUMBITS(2) [
                EL0_AND_EL1a = 0b0,
                EL0_ONLY = 0b01,
                EL0_AND_EL1b = 0b10,
                NONE = 0b11
            ]
        ]
    }
    pub struct Reg;

    impl RegisterReadWrite<u32, CPACR_EL1::Register> for Reg {
        sys_coproc_read_raw!(u32, "CPACR_EL1", "x");
        sys_coproc_write_raw!(u32, "CPACR_EL1", "x");
    }

    pub static CPACR_EL1: Reg = Reg {};
}
