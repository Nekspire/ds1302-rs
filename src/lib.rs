//! DS1302 platform agnostic driver crate
//!
//! # About
//!
//! DS1302 is a real time clock/calendar (RTCC) chip, which communicates with SPI interface.  
//! The device provides seconds, minutes, hours, day, date, month, and year information.
//!Driver is based on [`embedded-hal`] traits.
//! Datasheet: [DS1302](https://datasheets.maximintegrated.com/en/ds/DS1302.pdf)
//!
//! [`embedded-hal`]: https://github.com/rust-embedded/embedded-hal
//!
//! # The driver allows to:
//!
//! - Read and set clock and calendar data in 12-hour or 24-hour format .
//! - Changing hour format without reseting it. `set_clock_mode()`.
//!
//! # The driver does not allow to:
//!
//! - Currently using RAM is not supported.
//!
//! # Initialization
//!
//! ```
//! // External crates for IO and strings manipulation
//! use core::fmt::Write;
//! use heapless::String;
//! // DS1302 driver crate
//! use ds1302::{DS1302, Hours, Clock, Calendar, Mode as ds1302_mode};
//!
//! // Create with DS1302::new(), specify hour format mode: ds1302_mode::Hour12, in this case
//! let mut ds1302 = DS1302::new(spi, cs, ds1302_mode::Hour12).unwrap();
//!
//! ```
//!  # Read time and date
//! ```
//! let mut data = String::<U32>::from(" ");
//!
//! let cl = ds1302.get_clock_calendar().unwrap();
//! let (text, h) = match cl.0.hours {
//!     Hours::Hour24(h) => ("", h),
//!     Hours::Hour12am(h) => ("am", h),
//!     Hours::Hour12pm(h) => ("pm", h),
//! };
//! // Glue cl reads in string called "data", and use it later ...
//! let _=write!(data,"{} {}.{}.{}\n{:02}:{:02}:{:02} {}",
//!             cl.1.day, cl.1.date, cl.1.month, cl.1.year,
//!             h, cl.0.minutes, cl.0.seconds, text);
//! ```
//!
//!
//! From lib.rs:
//! ```
//! pub enum Hours {
//!     Hour24(u8),
//!     Hour12am(u8),
//!     Hour12pm(u8),
//! }
//! pub enum Mode {
//!     Hour24,
//!     Hour12,
//! }
//! ```
//!
//!
//! # Set time and date
//!
//! ```
//! let clk = Clock {
//!     hours: Hours::Hour12pm(4),
//!     minutes: 29,
//!     seconds: 0
//! };
//! let cal = Calendar {
//!     day: 2,
//!     date: 27,
//!     month: 10,
//!     year: 2020
//! };
//! ds1302.set_clock_calendar(clk, cal).unwrap();
//!
//! ```
//!
//!
//!
#![no_std]

use core::convert::From;
use embedded_hal as hal;
use fugit::ExtU32;
use hal::blocking::spi;
use hal::digital::v2::OutputPin;
use registers::Register;

const CLOCK_HALT_FLAG: u8 = 0x80;
const WRITE_PROTECT_BIT: u8 = 0x80;
const READ_BIT: u8 = 0x1;
const HOUR_12_BIT: u8 = 0x80;
const HOUR_PM_BIT: u8 = 0x20;

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
    pub fn new(spi: SPI, cs: CS, mode: Mode, timer: CLK) -> Result<Self, E> {
        let mut ds1302 = DS1302 { spi, cs, timer };
        // Check CLOCK HALT FLAG bit
        let byte = ds1302.read_reg(Register::SECONDS.addr())?;
        // Reset CLOCK HALT FLAG bit, power on device
        if (byte & CLOCK_HALT_FLAG) != 0 {
            ds1302.write_reg(Register::SECONDS.addr(), 0)?;
            let byte = ds1302.read_reg(Register::SECONDS.addr())?;
            if (byte & CLOCK_HALT_FLAG) != 0 {
                unimplemented!() // error condition
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
    pub fn destroy(self) -> Result<(SPI, CS, CLK), E> {
        Ok((self.spi, self.cs, self.timer))
    }

    fn read_reg(&mut self, reg: u8) -> Result<u8, E> {
        let mut bytes = [reg | READ_BIT, 0];
        nb::block!(self.timer.wait()).ok(); // wait CE inactive time min 4us
        self.cs.set_high().ok();
        self.spi.transfer(&mut bytes).ok();
        self.cs.set_low().ok();
        self.timer.start(4.micros()).ok();
        Ok(bytes[1])
    }

    fn write_reg(&mut self, reg: u8, byte: u8) -> Result<(), E> {
        //Firstly Check WRITE_PROTECT_BIT
        let wp_read = self.read_reg(Register::WP.addr())?;
        if (wp_read & WRITE_PROTECT_BIT) != 0 {
            let mut bytes = [Register::WP.addr(), 0];
            nb::block!(self.timer.wait()).ok(); // wait CE inactive time min 4us
            self.cs.set_high().ok();
            self.spi.write(&mut bytes).ok();
            self.cs.set_low().ok();
            self.timer.start(4.micros()).ok();
        }
        //Then write current data to registers
        let mut bytes = [reg, byte];
        nb::block!(self.timer.wait()).ok(); // wait CE inactive time min 4us
        self.cs.set_high().ok();
        self.spi.write(&mut bytes).ok();
        self.cs.set_low().ok();
        self.timer.start(4.micros()).ok();
        Ok(())
    }

    ///Return current information about seconds
    pub fn get_seconds(&mut self) -> Result<u8, E> {
        self.read_reg(Register::SECONDS.addr())
            .map(|b| bcd_to_decimal(b))
    }
    ///Return current information about minutes
    pub fn get_minutes(&mut self) -> Result<u8, E> {
        self.read_reg(Register::MINUTES.addr())
            .map(|b| bcd_to_decimal(b))
    }
    ///Return current information about hours
    pub fn get_hours(&mut self) -> Result<Hours, E> {
        self.read_reg(Register::HOURS.addr()).map(|b| b.into())
    }
    ///Return current information about date
    pub fn get_date(&mut self) -> Result<u8, E> {
        self.read_reg(Register::DATE.addr())
            .map(|b| bcd_to_decimal(b))
    }
    ///Return current information about month
    pub fn get_month(&mut self) -> Result<u8, E> {
        self.read_reg(Register::MONTH.addr())
            .map(|b| bcd_to_decimal(b))
    }
    ///Return current information about year
    pub fn get_year(&mut self) -> Result<u16, E> {
        self.read_reg(Register::YEAR.addr())
            .map(|b| 2000_u16 + (bcd_to_decimal(b) as u16))
    }
    ///Return current information about day of the week
    pub fn get_day(&mut self) -> Result<u8, E> {
        self.read_reg(Register::DAY.addr())
            .map(|b| bcd_to_decimal(b))
    }
    ///Return current information about hours, minutes and seconds
    pub fn get_clock(&mut self) -> Result<Clock, E> {
        let mut bytes = [0_u8; 4];
        bytes[0] = Register::CLKBURS.addr() | 1_u8;
        nb::block!(self.timer.wait()).ok(); // wait CE inactive time min 4us
        self.cs.set_high().ok();
        self.spi.transfer(&mut bytes)?;
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
    pub fn get_calendar(&mut self) -> Result<Calendar, E> {
        let mut bytes = [0_u8; 8];
        bytes[0] = Register::CLKBURS.addr() | 1_u8;
        nb::block!(self.timer.wait()).ok(); // wait CE inactive time min 4us
        self.cs.set_high().ok();
        self.spi.transfer(&mut bytes)?;
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
    pub fn get_clock_calendar(&mut self) -> Result<(Clock, Calendar), E> {
        let mut bytes = [0_u8; 8];
        bytes[0] = Register::CLKBURS.addr() | 1_u8;
        nb::block!(self.timer.wait()).ok(); // wait CE inactive time min 4us
        self.cs.set_high().ok();
        self.spi.transfer(&mut bytes)?;
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
    pub fn set_seconds(&mut self, seconds: u8) -> Result<(), E> {
        self.write_reg(Register::SECONDS.addr(), decimal_to_bcd(seconds))
    }
    ///Set minutes to defined value
    pub fn set_minutes(&mut self, minutes: u8) -> Result<(), E> {
        self.write_reg(Register::MINUTES.addr(), decimal_to_bcd(minutes))
    }
    ///Set hours to defined value
    pub fn set_hours(&mut self, hours: Hours) -> Result<(), E> {
        self.write_reg(Register::HOURS.addr(), hours.into())
    }
    ///Set date to defined value
    pub fn set_date(&mut self, date: u8) -> Result<(), E> {
        self.write_reg(Register::DATE.addr(), decimal_to_bcd(date))
    }
    ///Set month to defined value
    pub fn set_month(&mut self, month: u8) -> Result<(), E> {
        self.write_reg(Register::MONTH.addr(), decimal_to_bcd(month))
    }
    ///Set day of the week to defined value
    pub fn set_day(&mut self, day: u8) -> Result<(), E> {
        self.write_reg(Register::DAY.addr(), decimal_to_bcd(day))
    }
    ///Set year to defined value
    pub fn set_year(&mut self, year: u16) -> Result<(), E> {
        let y = if year < 2000 { 0 } else { year - 2000 };
        self.write_reg(Register::YEAR.addr(), decimal_to_bcd(y as u8))
    }
    ///Set clock to defined values
    pub fn set_clock(&mut self, clock: Clock) -> Result<(), E> {
        //Not burst mode, because it changes the calendar registers
        self.set_hours(clock.hours)?;
        self.set_minutes(clock.minutes)?;
        self.set_seconds(clock.seconds)
    }
    ///Set calendar to defined values
    pub fn set_calendar(&mut self, calendar: Calendar) -> Result<(), E> {
        //Not burst mode, because it changes the clock registers
        self.set_year(calendar.year)?;
        self.set_month(calendar.month)?;
        self.set_date(calendar.date)?;
        self.set_day(calendar.day)
    }
    ///Set clock and calendar to defined values
    pub fn set_clock_calendar(&mut self, clock: Clock, calendar: Calendar) -> Result<(), E> {
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
        self.spi.write(&mut bytes)?;
        self.cs.set_low().ok();
        self.timer.start(4.micros()).ok();
        Ok(())
    }
    ///Switch between 12-hour (AM/PM) and 24-hour mode
    pub fn set_clock_mode(&mut self, mode: Mode) -> Result<(), E> {
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
}

// Swap format from bcd to decmial
fn bcd_to_decimal(bcd: u8) -> u8 {
    ((bcd & 0xF0) >> 4) * 10 + (bcd & 0x0F)
}

// Swap format from decimal to bcd
fn decimal_to_bcd(decimal: u8) -> u8 {
    ((decimal / 10) << 4) + (decimal % 10)
}
