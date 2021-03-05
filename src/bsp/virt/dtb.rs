use dtb::Reader;

pub fn get_dtb(addr: &[u8]) -> Result<Reader, dtb::Error> {
    unsafe { Reader::read_from_address(addr.as_ptr() as usize) }
}
