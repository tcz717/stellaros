use core::fmt::Write;

#[inline(always)]
pub unsafe fn raw_print(s: &str) {
    const UART0: *mut u8 = 0x09000000 as *mut u8;
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
