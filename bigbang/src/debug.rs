use core::fmt;
use core::fmt::Write;

pub const UART0: *mut u8 = 0x09000000 as *mut u8;

#[inline(always)]
pub unsafe fn raw_print(s: &str) {
    for byte in s.as_bytes() {
        core::ptr::write_volatile(UART0, *byte);
    }
}

pub struct Console;

impl Write for Console {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        unsafe {
            raw_print(s);
        }
        Ok(())
    }
}

static mut CONSOLE: Console = Console;
#[inline(always)]
pub fn console() -> &'static mut dyn Write {
    unsafe { &mut CONSOLE }
}


#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    console().write_fmt(args).unwrap();
}

/// Prints without a newline.
///
/// Carbon copy from <https://doc.rust-lang.org/src/std/macros.rs.html>
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::debug::_print(format_args!($($arg)*)));
}

/// Prints with a newline.
///
/// Carbon copy from <https://doc.rust-lang.org/src/std/macros.rs.html>
#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ({
        $crate::debug::_print(format_args_nl!($($arg)*));
    })
}
