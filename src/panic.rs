use core::panic::PanicInfo;

#[panic_handler]
fn on_panic(info: &PanicInfo) -> ! {
    unsafe{core::ptr::write_volatile(0x0900_0000 as *mut u8,  b'p');}
    println!("{}", info);
    loop {}
}