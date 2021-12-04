//! DS1302 real time clock-calendar platform agnostic driver
//!
//! # About
//!
//!The DS1302 trickle-charge timekeeping chip contains a real-time clock/calendar and 31 bytes of static RAM. It
//!communicates with a microprocessor via a simple serial interface. The real-time clock/calendar provides seconds,
//!minutes, hours, day, date, month, and year information. The end of the month date is automatically adjusted for
//!months with fewer than 31 days, including corrections for leap year. The clock operates in either the 24-hour or
//!12-hour format with an AM/PM indicator. The chip driver is based on [`embedded-hal`] traits.
//!
//! [`embedded-hal`]: https://github.com/rust-embedded/embedded-hal
//!
//!Datasheet: [DS1302](https://datasheets.maximintegrated.com/en/ds/DS1302.pdf)
//!
//! ## Driver features:
//! - Reading/setting clock/calendar data
//! - 12-hour (AM/PM) or 24-hour format
//! - Changing the time format while the chip is working
//!
//!
//! NEW (4.0.0 release):
//! - Programmable Trickle Charger configuration
//! - 31 x 8 Battery-Backed General-Purpose RAM operations
//!

#![no_std]

use core::convert::From;
use embedded_hal as hal;
use fugit::ExtU32;
use hal::blocking::spi;
use hal::digital::v2::OutputPin;
pub use registers::{Ds, Rs};
use registers::{Register, TrickleCharger};

const CLOCK_HALT_FLAG: u8 = 0x80;
const WRITE_PROTECT_BIT: u8 = 0x80;
const READ_BIT: u8 = 0x1;
const HOUR_12_BIT: u8 = 0x80;
const HOUR_PM_BIT: u8 = 0x20;

/// DS1302 error
#[derive(Debug)]
pub enum Ds1302Error {
    Parameter,
    Spi,
    Unknown,
}

/// For timing `ds1302` uses [fugit](https://lib.rs/crates/fugit) crate which only provides `Duration` and `Instant` types.
/// It does not provide any clock or timer traits.
/// Therefore `ds1302` has its own `Delay` trait that provides all timing capabilities that are needed for the library.
/// User must implement this trait for the timer by itself.
pub trait Delay<const TIMER_HZ: u32> {
    /// An error that might happen during waiting
    type Error;

    /// Return current time `Instant`
    fn now(&mut self) -> fugit::TimerInstantU32<TIMER_HZ>;

    /// Start countdown with a `duration`
    fn start(&mut self, duration: fugit::TimerDurationU32<TIMER_HZ>) -> Result<(), Self::Error>;

    /// Wait until countdown `duration` has expired.
    /// Must return `nb::Error::WouldBlock` if countdown `duration` is not yet over.
    /// Must return `OK(())` as soon as countdown `duration` has expired.
    fn wait(&mut self) -> nb::Result<(), Self::Error>;
}

///DS1302 RTCC driver
pub struct DS1302<SPI, CS, CLK, const TIMER_HZ: u32>
where
    CLK: Delay<TIMER_HZ>,
{
    spi: SPI,
    cs: CS,
    timer: CLK,
}
///Hour format: 12-hour (AM/PM) or 24-hour
#[derive(PartialEq)]
pub enum Mode {
    Hour24,
    Hour12,
}
///Hour information: 12-hour (AM/PM) or 24-hour
pub enum Hours {
    Hour24(u8),
    Hour12am(u8),
    Hour12pm(u8),
}

impl Hours {
    fn convert(&self) -> Self {
        match *self {
            Hours::Hour24(h) => {
                if h >= 12 {
                    Hours::Hour12pm(h - 12)
                } else {
                    Hours::Hour12am(h)
                }
            }
            Hours::Hour12pm(h) => Hours::Hour24(h + 12),
            Hours::Hour12am(h) => Hours::Hour24(h),
        }
    }

    /// Get the hour.
    /// return.1: None => Hour24 mode; Some(false) => pm; Some(true) => am;
    pub fn hour(&self) -> (u8, Option<bool>) {
        match *self {
            Hours::Hour24(h) => (h, None),
            Hours::Hour12am(h) => (h, Some(false)),
            Hours::Hour12pm(h) => (h, Some(true)),
        }
    }
}

impl From<u8> for Hours {
    fn from(byte: u8) -> Self {
        if (byte & HOUR_12_BIT) != 0 {
            //In case 12-hour format
            let hour = bcd_to_decimal(byte & (!(HOUR_12_BIT | HOUR_PM_BIT)));
            if (byte & HOUR_PM_BIT) != 0 {
                // It's PM
                Hours::Hour12pm(hour)
            } else {
                // It's AM
                Hours::Hour12am(hour)
            }
        } else {
            let hour = bcd_to_decimal(byte);
            Hours::Hour24(hour)
        }
    }
}

impl From<Hours> for u8 {
    fn from(h: Hours) -> Self {
        match h {
            Hours::Hour24(hour) => decimal_to_bcd(hour),
            Hours::Hour12am(hour) => decimal_to_bcd(hour) | HOUR_12_BIT,
            Hours::Hour12pm(hour) => decimal_to_bcd(hour) | HOUR_12_BIT | HOUR_PM_BIT,
        }
    }
}

///Clock information
pub struct Clock {
    pub hours: Hours,
    pub minutes: u8,
    pub seconds: u8,
}
///Calendar information
pub struct Calendar {
    pub day: u8,
    pub date: u8,
    pub month: u8,
    pub year: u16,
}

mod registers;

impl<SPI, CS, E, PinError, CLK, const TIMER_HZ: u32> DS1302<SPI, CS, CLK, TIMER_HZ>
where
    SPI: spi::Transfer<u8, Error = E> + spi::Write<u8, Error = E>,
    CS: OutputPin<Error = PinError>,
    CLK: Delay<TIMER_HZ>,
{
    ///Creates new instance DS1302 RTC
    pub fn new(spi: SPI, cs: CS, mode: Mode, timer: CLK) -> Result<Self, Ds1302Error> {
        let mut ds1302 = DS1302 { spi, cs, timer };
        // Check CLOCK HALT FLAG bit
        let byte = ds1302.read_reg(Register::SECONDS.addr())?;
        // Reset CLOCK HALT FLAG bit, power on device
        if (byte & CLOCK_HALT_FLAG) != 0 {
            ds1302.write_reg(Register::SECONDS.addr(), 0)?;
            let byte = ds1302.read_reg(Register::SECONDS.addr())?;
            if (byte & CLOCK_HALT_FLAG) != 0 {
                Err(Ds1302Error::Unknown)
            } else {
                ds1302.set_clock_mode(mode)?;
                Ok(ds1302)
            }
        } else {
            ds1302.set_clock_mode(mode)?;
            Ok(ds1302)
        }
    }
    ///Delete DS1302 RTC instance and return SPI interface and cs PIN
    pub fn destroy(self) -> Result<(SPI, CS, CLK), Ds1302Error> {
        Ok((self.spi, self.cs, self.timer))
    }

    fn read_reg(&mut self, reg: u8) -> Result<u8, Ds1302Error> {
        let mut bytes = [reg | READ_BIT, 0];
        nb::block!(self.timer.wait()).ok(); // wait CE inactive time min 4us
        self.cs.set_high().ok();
        self.spi
            .transfer(&mut bytes)
            .map_err(|_| Ds1302Error::Spi)?;
        self.cs.set_low().ok();
        self.timer.start(4.micros()).ok();
        Ok(bytes[1])
    }

    fn write_reg(&mut self, reg: u8, byte: u8) -> Result<(), Ds1302Error> {
        //Firstly Check WRITE_PROTECT_BIT
        let wp_read = self.read_reg(Register::WP.addr())?;
        if (wp_read & WRITE_PROTECT_BIT) != 0 {
            let mut bytes = [Register::WP.addr(), 0];
            nb::block!(self.timer.wait()).ok(); // wait CE inactive time min 4us
            self.cs.set_high().ok();
            self.spi.write(&mut bytes).map_err(|_| Ds1302Error::Spi)?;
            self.cs.set_low().ok();
            self.timer.start(4.micros()).ok();
        }
        //Then write current data to registers
        let mut bytes = [reg, byte];
        nb::block!(self.timer.wait()).ok(); // wait CE inactive time min 4us
        self.cs.set_high().ok();
        self.spi.write(&mut bytes).map_err(|_| Ds1302Error::Spi)?;
        self.cs.set_low().ok();
        self.timer.start(4.micros()).ok();
        Ok(())
    }

    ///Return current information about seconds
    pub fn get_seconds(&mut self) -> Result<u8, Ds1302Error> {
        self.read_reg(Register::SECONDS.addr())
            .map(|b| bcd_to_decimal(b))
    }
    ///Return current information about minutes
    pub fn get_minutes(&mut self) -> Result<u8, Ds1302Error> {
        self.read_reg(Register::MINUTES.addr())
            .map(|b| bcd_to_decimal(b))
    }
    ///Return current information about hours
    pub fn get_hours(&mut self) -> Result<Hours, Ds1302Error> {
        self.read_reg(Register::HOURS.addr()).map(|b| b.into())
    }
    ///Return current information about date
    pub fn get_date(&mut self) -> Result<u8, Ds1302Error> {
        self.read_reg(Register::DATE.addr())
            .map(|b| bcd_to_decimal(b))
    }
    ///Return current information about month
    pub fn get_month(&mut self) -> Result<u8, Ds1302Error> {
        self.read_reg(Register::MONTH.addr())
            .map(|b| bcd_to_decimal(b))
    }
    ///Return current information about year
    pub fn get_year(&mut self) -> Result<u16, Ds1302Error> {
        self.read_reg(Register::YEAR.addr())
            .map(|b| 2000_u16 + (bcd_to_decimal(b) as u16))
    }
    ///Return current information about day of the week
    pub fn get_day(&mut self) -> Result<u8, Ds1302Error> {
        self.read_reg(Register::DAY.addr())
            .map(|b| bcd_to_decimal(b))
    }
    ///Return current information about hours, minutes and seconds
    pub fn get_clock(&mut self) -> Result<Clock, Ds1302Error> {
        let mut bytes = [0_u8; 4];
        bytes[0] = Register::CLKBURS.addr() | 1_u8;
        nb::block!(self.timer.wait()).ok(); // wait CE inactive time min 4us
        self.cs.set_high().ok();
        self.spi
            .transfer(&mut bytes)
            .map_err(|_| Ds1302Error::Spi)?;
        self.cs.set_low().ok();
        self.timer.start(4.micros()).ok();

        let clock = Clock {
            seconds: bcd_to_decimal(bytes[1]),
            minutes: bcd_to_decimal(bytes[2]),
            hours: bytes[3].into(),
        };

        Ok(clock)
    }
    ///Return current information about date, day of the week, month and year
    pub fn get_calendar(&mut self) -> Result<Calendar, Ds1302Error> {
        let mut bytes = [0_u8; 8];
        bytes[0] = Register::CLKBURS.addr() | 1_u8;
        nb::block!(self.timer.wait()).ok(); // wait CE inactive time min 4us
        self.cs.set_high().ok();
        self.spi
            .transfer(&mut bytes)
            .map_err(|_| Ds1302Error::Spi)?;
        self.cs.set_low().ok();
        self.timer.start(4.micros()).ok();

        let calendar = Calendar {
            date: bcd_to_decimal(bytes[4]),
            day: bcd_to_decimal(bytes[5]),
            month: bcd_to_decimal(bytes[6]),
            year: (2000_u16 + (bcd_to_decimal(bytes[7]) as u16)),
        };

        Ok(calendar)
    }
    ///Return current information date and time
    pub fn get_clock_calendar(&mut self) -> Result<(Clock, Calendar), Ds1302Error> {
        let mut bytes = [0_u8; 8];
        bytes[0] = Register::CLKBURS.addr() | 1_u8;
        nb::block!(self.timer.wait()).ok(); // wait CE inactive time min 4us
        self.cs.set_high().ok();
        self.spi
            .transfer(&mut bytes)
            .map_err(|_| Ds1302Error::Spi)?;
        self.cs.set_low().ok();
        self.timer.start(4.micros()).ok();

        let clock = Clock {
            seconds: bcd_to_decimal(bytes[1]),
            minutes: bcd_to_decimal(bytes[2]),
            hours: bytes[3].into(),
        };

        let calendar = Calendar {
            date: bcd_to_decimal(bytes[4]),
            month: bcd_to_decimal(bytes[5]),
            day: bcd_to_decimal(bytes[6]),
            year: (2000_u16 + (bcd_to_decimal(bytes[7]) as u16)),
        };

        Ok((clock, calendar))
    }
    ///Set seconds to defined value
    pub fn set_seconds(&mut self, seconds: u8) -> Result<(), Ds1302Error> {
        self.write_reg(Register::SECONDS.addr(), decimal_to_bcd(seconds))
    }
    ///Set minutes to defined value
    pub fn set_minutes(&mut self, minutes: u8) -> Result<(), Ds1302Error> {
        self.write_reg(Register::MINUTES.addr(), decimal_to_bcd(minutes))
    }
    ///Set hours to defined value
    pub fn set_hours(&mut self, hours: Hours) -> Result<(), Ds1302Error> {
        self.write_reg(Register::HOURS.addr(), hours.into())
    }
    ///Set date to defined value
    pub fn set_date(&mut self, date: u8) -> Result<(), Ds1302Error> {
        self.write_reg(Register::DATE.addr(), decimal_to_bcd(date))
    }
    ///Set month to defined value
    pub fn set_month(&mut self, month: u8) -> Result<(), Ds1302Error> {
        self.write_reg(Register::MONTH.addr(), decimal_to_bcd(month))
    }
    ///Set day of the week to defined value
    pub fn set_day(&mut self, day: u8) -> Result<(), Ds1302Error> {
        self.write_reg(Register::DAY.addr(), decimal_to_bcd(day))
    }
    ///Set year to defined value
    pub fn set_year(&mut self, year: u16) -> Result<(), Ds1302Error> {
        let y = if year < 2000 { 0 } else { year - 2000 };
        self.write_reg(Register::YEAR.addr(), decimal_to_bcd(y as u8))
    }
    ///Set clock to defined values
    pub fn set_clock(&mut self, clock: Clock) -> Result<(), Ds1302Error> {
        //Not burst mode, because it changes the calendar registers
        self.set_hours(clock.hours)?;
        self.set_minutes(clock.minutes)?;
        self.set_seconds(clock.seconds)
    }
    ///Set calendar to defined values
    pub fn set_calendar(&mut self, calendar: Calendar) -> Result<(), Ds1302Error> {
        //Not burst mode, because it changes the clock registers
        self.set_year(calendar.year)?;
        self.set_month(calendar.month)?;
        self.set_date(calendar.date)?;
        self.set_day(calendar.day)
    }
    ///Set clock and calendar to defined values
    pub fn set_clock_calendar(
        &mut self,
        clock: Clock,
        calendar: Calendar,
    ) -> Result<(), Ds1302Error> {
        //Writing in burst mode, it changes all the clock and calendar registers
        let mut bytes = [0_u8; 9];
        bytes[0] = Register::CLKBURS.addr();
        bytes[1] = decimal_to_bcd(clock.seconds);
        bytes[2] = decimal_to_bcd(clock.minutes);
        bytes[3] = clock.hours.into();
        bytes[4] = decimal_to_bcd(calendar.date);
        bytes[5] = decimal_to_bcd(calendar.month);
        bytes[6] = decimal_to_bcd(calendar.day);
        let y = if calendar.year < 2000 {
            0
        } else {
            calendar.year - 2000
        };
        bytes[7] = decimal_to_bcd(y as u8);

        nb::block!(self.timer.wait()).ok(); // wait CE inactive time min 4us
        self.cs.set_high().ok();
        self.spi.write(&mut bytes).map_err(|_| Ds1302Error::Spi)?;
        self.cs.set_low().ok();
        self.timer.start(4.micros()).ok();
        Ok(())
    }
    ///Switch between 12-hour (AM/PM) and 24-hour mode
    pub fn set_clock_mode(&mut self, mode: Mode) -> Result<(), Ds1302Error> {
        let hr = self.get_hours()?; // save current hours data
        match hr {
            Hours::Hour24(_h) => {
                if mode == Mode::Hour12 {
                    self.set_hours(hr.convert())
                } else {
                    Ok(())
                }
            }
            _ => {
                if mode == Mode::Hour24 {
                    self.set_hours(hr.convert())
                } else {
                    Ok(())
                }
            }
        }
    }

    /// Enable trickle-charge.
    /// Ds (diode drop voltage 0.7 or 1.4)
    /// Rs (2k or 4k or 8k)
    /// The maximum current = (Vcc - Ds) / Rs.
    pub fn tc_enable(&mut self, ds: Ds, rs: Rs) -> Result<(), Ds1302Error> {
        self.write_reg(Register::TCS.addr(), TrickleCharger::enable(ds, rs))
    }

    /// Disable trickle-charge.
    pub fn tc_disable(&mut self) -> Result<(), Ds1302Error> {
        self.write_reg(Register::TCS.addr(), TrickleCharger::disable())
    }

    /// Get the configuration of the trickle-charge register.
    pub fn tc_get(&mut self) -> Result<(bool, Option<Ds>, Option<Rs>), Ds1302Error> {
        let v = self.read_reg(Register::TCS.addr())?;
        Ok(TrickleCharger::from(v).get())
    }

    /// Whether to enable charging.
    pub fn tc_is_enabled(&mut self) -> Result<bool, Ds1302Error> {
        let v = self.read_reg(Register::TCS.addr())?;
        Ok(TrickleCharger::from(v).is_enabled())
    }

    /// Read DS1302 internal RAM. The static RAM is 31 x 8 bytes, index 0..=30.
    pub fn read_ram(&mut self, index: u8) -> Result<u8, Ds1302Error> {
        if index > 30 {
            return Err(Ds1302Error::Parameter);
        }
        self.read_reg(Register::RAM.addr() + index * 2)
    }

    /// Write DS1302 internal RAM. The static RAM is 31 x 8 bytes, index 0..=31.
    pub fn write_ram(&mut self, index: u8, value: u8) -> Result<(), Ds1302Error> {
        if index > 30 {
            return Err(Ds1302Error::Parameter);
        }
        self.write_reg(Register::RAM.addr() + index * 2, value)
    }

    /// Read DS1302 internal RAM burst mode. Start at 0 index.
    /// The length is determined by the buf, but cannot exceed 31.
    pub fn read_ram_burst(&mut self, buf: &mut [u8]) -> Result<(), Ds1302Error> {
        let mut bytes = [0_u8; 32];
        bytes[0] = Register::RAMBURS.addr() | 1_u8;
        nb::block!(self.timer.wait()).ok(); // wait CE inactive time min 4us
        self.cs.set_high().ok();
        self.spi
            .transfer(&mut bytes[..(buf.len() + 1)])
            .map_err(|_| Ds1302Error::Spi)?;
        self.cs.set_low().ok();
        self.timer.start(4.micros()).ok();
        buf.copy_from_slice(&bytes[1..(buf.len() + 1)]);
        Ok(())
    }

    /// Write DS1302 internal RAM burst mode. Start at 0 index.
    /// The length is determined by the buf, but cannot exceed 31.
    pub fn write_ram_burst(&mut self, buf: &[u8]) -> Result<usize, Ds1302Error> {
        let mut bytes = [0_u8; 32];
        bytes[0] = Register::RAMBURS.addr();
        let ll = buf.len();
        let ll = if ll > 31 { 31 } else { ll };
        bytes[1..(ll + 1)].copy_from_slice(&buf[..ll]);

        nb::block!(self.timer.wait()).ok(); // wait CE inactive time min 4us
        self.cs.set_high().ok();
        self.spi
            .write(&mut bytes[..(ll + 1)])
            .map_err(|_| Ds1302Error::Spi)?;
        self.cs.set_low().ok();
        self.timer.start(4.micros()).ok();
        Ok(ll)
    }
}

// Swap format from bcd to decmial
fn bcd_to_decimal(bcd: u8) -> u8 {
    ((bcd & 0xF0) >> 4) * 10 + (bcd & 0x0F)
}

// Swap format from decimal to bcd
fn decimal_to_bcd(decimal: u8) -> u8 {
    ((decimal / 10) << 4) + (decimal % 10)
}
