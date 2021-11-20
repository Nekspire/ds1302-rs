/// Register definitions
pub enum Register {
    SECONDS = 0x80,
    MINUTES = 0x82,
    HOURS = 0x84,
    DATE = 0x86,
    MONTH = 0x88,
    DAY = 0x8A,
    YEAR = 0x8C,
    WP = 0x8E,
    TCS = 0x90,
    CLKBURS = 0xBE,
    RAM = 0xC0,
    RAMBURS = 0xFE,
}

impl Register {
    pub fn addr(self) -> u8 {
        self as u8
    }
}
