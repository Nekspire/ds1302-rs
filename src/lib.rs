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
//! let mut text = " ";
//!
//! let cl = ds1302.get_clock_calendar().unwrap();
//! // Check the current mode. If it is Hour12, check AM/PM value. Please refer to table description below.
//! match ds1302.mode {
//!     ds1302_mode::Hour12 => {
//!         if cl.0.hours.am_pm == 1 {text = " PM"}
//!         else {text = " AM"}
//!     }
//!     ds1302_mode::Hour24 => text = ""
//! }
//! // Glue cl reads in string called "data", and use it later ...
//! let _=write!(data,"{} {}.{}.{}\n{:02}:{:02}:{:02} {}",
//!             cl.1.day, cl.1.date, cl.1.month, cl.1.year,
//!             cl.0.hours.hours, cl.0.minutes, cl.0.seconds, text);
//! ```
//! 
//! 
//! From lib.rs:
//! ```
//! pub struct Hours {
//!     pub hours: u8,
//!     pub am_pm: u8,
//! } 
//! 
//! pub enum Mode {
//!     Hour24,
//!     Hour12,
//! }
//! ```
//! Variants of time format depending on Mode::Hour24, Mode::Hour12 and Hours::am_mp
//! 
//! Mode | am_pm | time format
//! --- | --- | ---
//! Hour12 | 0 | AM
//! Hour12 | 1 | PM
//! Hour24 | 0 | -
//! Hour24 | 1 | -
//! 
//! 
//! # Set time and date
//! 
//! ```
//! let h = Hours {hours: 4, am_pm: 1};
//! let clk = Clock {
//!     hours: h,
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

use embedded_hal as hal;
use hal::blocking::spi;
use hal::digital::v2::OutputPin;
use registers::Register;

const CLOCK_HALT_FLAG: u8 =     0x80;
const WRITE_PROTECT_BIT: u8 =   0x80;
const READ_BIT: u8 =            0x1;
const HOUR_12_BIT: u8 =         0x80;
const HOUR_PM_BIT: u8 =         0x20;
///DS1302 RTCC driver
pub struct DS1302<SPI , CS> {
    spi: SPI,
    cs: CS,
    pub mode: Mode
}
///Hour format: 12-hour (AM/PM) or 24-hour
pub enum Mode {
    Hour24,
    Hour12,
}
///Hour information: 12-hour (AM/PM) or 24-hour
pub struct Hours {
    pub hours: u8,
    pub am_pm: u8,
}
///Clock information
pub struct Clock {
    pub hours: Hours,
    pub minutes: u8,
    pub seconds: u8
}
///Calendar information
pub struct Calendar {
    pub day: u8,
    pub date: u8,
    pub month: u8,
    pub year: u16
}

mod registers;

impl <SPI, CS, E, PinError> DS1302<SPI, CS>
where 
    SPI: spi::Transfer<u8, Error = E> + spi::Write<u8, Error = E>,
    CS: OutputPin<Error = PinError>
{
    ///Creates new instance DS1302 RTC
    pub fn new(spi: SPI, cs: CS, mode: Mode) -> Result<Self, E> {
        let mut ds1302 = DS1302 {spi, cs, mode: Mode::Hour12};
        // Check CLOCK HALT FLAG bit
        let byte = ds1302.read_reg(Register::SECONDS.addr())?;
        // Reset CLOCK HALT FLAG bit, power on device
        if (byte & CLOCK_HALT_FLAG) != 0 {
            ds1302.write_reg(Register::SECONDS.addr(), 0)?;
            let byte = ds1302.read_reg(Register::SECONDS.addr())?;
            if (byte & CLOCK_HALT_FLAG) != 0 {
                unimplemented!() // error condition
            }
            else {
                ds1302.set_clock_mode(mode)?;
                Ok(ds1302)
            }
        }
        else {
            ds1302.set_clock_mode(mode)?;
            Ok(ds1302)
        }
    }
    ///Delete DS1302 RTC instance and return SPI interface and cs PIN
    pub fn destroy(self) -> Result<(SPI,CS), E> {
        Ok((self.spi, self.cs))
    }

    fn read_reg(&mut self, reg: u8) -> Result<u8, E>  {
        let mut bytes = [reg | READ_BIT, 0];
        self.cs.set_high().ok();
        self.spi.transfer(&mut bytes).ok();
        self.cs.set_low().ok();
        Ok(bytes[1])
    }

    fn write_reg(&mut self, reg: u8, byte: u8) -> Result<(), E> { 
        self.bcd_to_decimal(byte)?;
        //Firstly Check WRITE_PROTECT_BIT
        let wp_read = self.read_reg(Register::WP.addr())?;
        if(wp_read & WRITE_PROTECT_BIT) != 0 {
            let mut bytes = [Register::WP.addr(), 0];
            self.cs.set_high().ok();
            self.spi.write(&mut bytes).ok();
            self.cs.set_low().ok();
        }
        //Then write current data to registers
        let mut bytes = [reg, byte];
        self.cs.set_high().ok();
        self.spi.write(&mut bytes).ok();
        self.cs.set_low().ok();
        Ok(())
    }
    // Swap format from bcd to decmial
    fn bcd_to_decimal(&mut self, bcd: u8) -> Result<u8, E> {
        Ok(((bcd & 0xF0) >> 4) * 10 + (bcd & 0x0F))
    }
    // Swap format from decimal to bcd
    fn decimal_to_bcd(&mut self, decimal: u8) -> Result<u8, E> {
        Ok(((decimal / 10) << 4) + (decimal % 10))
    }
    ///Return current information about seconds
    pub fn get_seconds(&mut self) -> Result<u8, E> {
        let byte = self.read_reg(Register::SECONDS.addr())?;
        self.bcd_to_decimal(byte)
    }
    ///Return current information about minutes
    pub fn get_minutes(&mut self) -> Result<u8, E> {
        let byte = self.read_reg(Register::MINUTES.addr())?;
        self.bcd_to_decimal(byte)
    }
    ///Return current information about hours
    pub fn get_hours(&mut self) -> Result<Hours, E> {
        let mut hr = Hours {
            hours:  0,
            am_pm: 0
        };
        let mut byte = self.read_reg(Register::HOURS.addr())?;
        if (byte & HOUR_12_BIT) != 0 {
            //In case 12-hour format
            if(byte & HOUR_PM_BIT) != 0 {
                // It's PM
                byte &= !(HOUR_12_BIT | HOUR_PM_BIT); // Clear 7th and 5th bits to designate hours 
                hr.am_pm = 1;
            } else {
                // It's AM
                byte &= !(HOUR_12_BIT); // Clear 7th bit to designate hours 
            }
        }
        hr.hours = self.bcd_to_decimal(byte)?;
        Ok(hr)
    }
    ///Return current information about date
    pub fn get_date(&mut self) -> Result<u8, E> {
        let byte = self.read_reg(Register::DATE.addr())?;
        self.bcd_to_decimal(byte)
    }
    ///Return current information about month
    pub fn get_month(&mut self) -> Result<u8, E> {
        let byte = self.read_reg(Register::MONTH.addr())?;
        self.bcd_to_decimal(byte)
    }
    ///Return current information about year
    pub fn get_year(&mut self) -> Result<u16, E> {
        let byte = self.read_reg(Register::YEAR.addr())?;
        Ok(2000_u16 + (self.bcd_to_decimal(byte)? as u16))
    }
    ///Return current information about day of the week
    pub fn get_day(&mut self) -> Result<u8, E> {
        let byte = self.read_reg(Register::DAY.addr())?;
        self.bcd_to_decimal(byte)
    }
    ///Return current information about hours, minutes and seconds
    pub fn get_clock(&mut self) -> Result<Clock, E> {
        let mut bytes = [0_u8; 4];
        bytes[0] = Register::CLKBURS.addr() | 1_u8;
        self.cs.set_high().ok();
        self.spi.transfer(&mut bytes)?;
        self.cs.set_low().ok();

        let mut hr = Hours {
            hours:  0,
            am_pm: 0
        };
        if (bytes[3] & HOUR_12_BIT) != 0 {
            //In case 12-hour format
            if(bytes[3] & HOUR_PM_BIT) != 0 {
                // It's PM
                bytes[3] &= !(HOUR_12_BIT | HOUR_PM_BIT); // Clear 7th and 5th bits to designate hours 
                hr.am_pm = 1;
            } else {
                // It's AM
                bytes[3] &= !(HOUR_12_BIT); // Clear 7th bit to designate hours 
            }
        }
        hr.hours = self.bcd_to_decimal(bytes[3])?;
        let clock = Clock {
            seconds: self.bcd_to_decimal(bytes[1])?,
            minutes: self.bcd_to_decimal(bytes[2])?,
            hours: hr,
        };

        Ok(clock)
    }
    ///Return current information about date, day of the week, month and year
    pub fn get_calendar(&mut self) -> Result<Calendar, E> {
        let mut bytes = [0_u8; 8];
        bytes[0] = Register::CLKBURS.addr() | 1_u8;
        self.cs.set_high().ok();
        self.spi.transfer(&mut bytes)?;
        self.cs.set_low().ok();
        
        let calendar = Calendar {
            date: self.bcd_to_decimal(bytes[4])?,
            day: self.bcd_to_decimal(bytes[5])?,
            month: self.bcd_to_decimal(bytes[6])?,
            year: (2000_u16 + (self.bcd_to_decimal(bytes[7])? as u16)),
        };

        Ok(calendar)
    }
    ///Return current information date and time
    pub fn get_clock_calendar(&mut self) -> Result<(Clock, Calendar), E> {
        let mut bytes = [0_u8; 8];
        bytes[0] = Register::CLKBURS.addr() | 1_u8;
        self.cs.set_high().ok();
        self.spi.transfer(&mut bytes)?;
        self.cs.set_low().ok();

        let mut hr = Hours {
            hours:  0,
            am_pm: 0
        };
        if (bytes[3] & HOUR_12_BIT) != 0 {
            //In case 12-hour format
            if(bytes[3] & HOUR_PM_BIT) != 0 {
                // It's PM
                bytes[3] &= !(HOUR_12_BIT | HOUR_PM_BIT); // Clear 7th and 5th bits to designate hours 
                hr.am_pm = 1;
            } else {
                // It's AM
                bytes[3] &= !(HOUR_12_BIT); // Clear 7th bit to designate hours 
            }
        }
        hr.hours = self.bcd_to_decimal(bytes[3])?;
        let clock = Clock {
            seconds: self.bcd_to_decimal(bytes[1])?,
            minutes: self.bcd_to_decimal(bytes[2])?,
            hours: hr,
        }; 

        let calendar = Calendar {
            date: self.bcd_to_decimal(bytes[4])?,
            month: self.bcd_to_decimal(bytes[5])?,
            day: self.bcd_to_decimal(bytes[6])?,
            year: (2000_u16 + (self.bcd_to_decimal(bytes[7])? as u16)),
        };

        Ok((clock, calendar))
    }
    ///Set seconds to defined value
    pub fn set_seconds(&mut self, seconds: u8) -> Result<(),E> {
        let byte = self.decimal_to_bcd(seconds)?; 
        self.write_reg(Register::SECONDS.addr(), byte)?;
        Ok(())
    }
    ///Set minutes to defined value
    pub fn set_minutes(&mut self, minutes: u8) -> Result<(),E> {
        let byte = self.decimal_to_bcd(minutes)?; 
        self.write_reg(Register::MINUTES.addr(), byte)?;
        Ok(())
    }
    ///Set hours to defined value
    pub fn set_hours(&mut self, hours: Hours) -> Result<(),E> {
        let mut byte = self.decimal_to_bcd(hours.hours)?;
        match self.mode {
            Mode::Hour12 => {
                if hours.am_pm == 1 {
                    byte |= HOUR_PM_BIT | HOUR_12_BIT;
                } else {
                    byte |= HOUR_12_BIT;
                }
            }
            Mode::Hour24 => {}
        }
        self.write_reg(Register::HOURS.addr(), byte)?;
        Ok(())
    }
    ///Set date to defined value
    pub fn set_date(&mut self, date: u8) -> Result<(),E> {
        let byte = self.decimal_to_bcd(date)?; 
        self.write_reg(Register::DATE.addr(), byte)?;
        Ok(())
    }
    ///Set month to defined value
    pub fn set_month(&mut self, month: u8) -> Result<(),E> {
        let byte = self.decimal_to_bcd(month)?; 
        self.write_reg(Register::MONTH.addr(), byte)?;
        Ok(())
    }
    ///Set day of the week to defined value
    pub fn set_day(&mut self, day: u8) -> Result<(),E> {
        let byte = self.decimal_to_bcd(day)?; 
        self.write_reg(Register::DAY.addr(), byte)?;
        Ok(())
    }
    ///Set year to defined value
    pub fn set_year(&mut self, mut year: u16) -> Result<(),E> {
        if year < 2000 {year = 2000}
        year -= 2000;
        let byte = self.decimal_to_bcd(year as u8)?; 
        self.write_reg(Register::YEAR.addr(), byte)?;
        Ok(())
    }
    ///Set clock to defined values
    pub fn set_clock(&mut self, clock: Clock) -> Result<(),E> {
         //Not burst mode, because it changes the calendar registers
        self.set_hours(clock.hours)?;
        self.set_minutes(clock.minutes)?;
        self.set_seconds(clock.seconds)?;
        Ok(())
    }
    ///Set calendar to defined values
    pub fn set_calendar(&mut self, calendar: Calendar) -> Result<(),E> {
        //Not burst mode, because it changes the clock registers
        self.set_year(calendar.year)?;
        self.set_month(calendar.month)?;
        self.set_date(calendar.date)?;
        self.set_day(calendar.day)?;
        Ok(())
    }
    ///Set clock and calendar to defined values
    pub fn set_clock_calendar(&mut self, clock: Clock, mut calendar: Calendar) -> Result<(),E> {
        //Writing in burst mode, it changes all the clock and calendar registers
        let mut bytes = [0_u8; 9];
        bytes[0] = Register::CLKBURS.addr();
        bytes[1] = self.decimal_to_bcd(clock.seconds)?;
        bytes[2] = self.decimal_to_bcd(clock.minutes)?;
        bytes[3] = self.decimal_to_bcd(clock.hours.hours)?;
        match self.mode {
            Mode::Hour12 => {
                if clock.hours.am_pm == 1 {
                    bytes[3] |= HOUR_PM_BIT | HOUR_12_BIT;
                } else {
                    bytes[3] |= HOUR_12_BIT;
                }
            }
            Mode::Hour24 => {}
        }
        bytes[4] = self.decimal_to_bcd(calendar.date)?;
        bytes[5] = self.decimal_to_bcd(calendar.month)?;
        bytes[6] = self.decimal_to_bcd(calendar.day)?;
        if calendar.year < 2000 {calendar.year = 2000}
        calendar.year -= 2000;
        bytes[7] = self.decimal_to_bcd(calendar.year as u8)?;
        self.cs.set_high().ok();
        self.spi.write(&mut bytes)?;
        self.cs.set_low().ok();
        Ok(())
    }
    ///Switch between 12-hour (AM/PM) and 24-hour mode
    pub fn set_clock_mode(&mut self, mode: Mode) -> Result<(), E> {
        let hr = self.get_hours()?; // save current hours data
        match mode {
            Mode::Hour12 => {
                self.write_reg(Register::HOURS.addr(), HOUR_12_BIT)?;
            }
            Mode::Hour24 => {
                self.write_reg(Register::HOURS.addr(), 0)?;
            }
        };
        let hr_new_mode = change_hour_to(&mode, hr);
        self.mode = mode;
        self.set_hours(hr_new_mode)?;
        Ok(())
    }
}


fn change_hour_to(mode: &Mode,mut hour: Hours) -> Hours {
    match mode {
        Mode::Hour24 => {
            if hour.am_pm == 1 {
                hour.hours += 12;
                hour.am_pm = 0;
            }
            hour
        }
        Mode::Hour12 => {
            if hour.hours > 12 {
                hour.hours -= 12;
                hour.am_pm = 1;
            }
            hour
        }
    }
}