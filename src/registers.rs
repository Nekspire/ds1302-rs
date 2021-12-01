use core::convert::From;

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

/// Programmable Trickle Charger.
pub(crate) struct TrickleCharger(u8);

/// Trickle charger resistor select.
pub enum Rs {
    R2K,
    R4K,
    R8K,
}

impl Rs {
    pub(crate) fn judge(b: u8) -> Option<Self> {
        match b & 0x03 {
            0x01 => Some(Rs::R2K),
            0x02 => Some(Rs::R4K),
            0x03 => Some(Rs::R8K),
            _ => None,
        }
    }

    pub(crate) fn value(&self) -> u8 {
        match self {
            Rs::R2K => 0x01,
            Rs::R4K => 0x02,
            Rs::R8K => 0x03,
        }
    }
}

/// Trickle charger diode select. diode drop 0.7v or 1.4v.
pub enum Ds {
    ONE07V = 0x04,
    TWO14V = 0x08,
}

impl Ds {
    pub(crate) fn judge(b: u8) -> Option<Self> {
        match b & 0x0C {
            0x04 => Some(Ds::ONE07V),
            0x08 => Some(Ds::TWO14V),
            _ => None,
        }
    }

    pub(crate) fn value(&self) -> u8 {
        match self {
            Ds::ONE07V => 0x04,
            Ds::TWO14V => 0x08,
        }
    }
}

impl TrickleCharger {
    pub fn get(&self) -> (bool, Option<Ds>, Option<Rs>) {
        let rs = Rs::judge(self.0);
        let ds = Ds::judge(self.0);
        let tcs = if rs.is_none() || ds.is_none() || (self.0 & 0xF0 != 0xA0) {
            false
        } else {
            true
        };

        (tcs, ds, rs)
    }

    pub fn is_enabled(&self) -> bool {
        self.get().0
    }

    pub fn disable() -> u8 {
        0x5C
    }

    pub fn enable(ds: Ds, rs: Rs) -> u8 {
        rs.value() | ds.value() | 0xA0
    }
}

impl From<u8> for TrickleCharger {
    fn from(b: u8) -> TrickleCharger {
        TrickleCharger(b)
    }
}
