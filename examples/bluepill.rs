#![no_main]
#![no_std]

use panic_halt as _;
use cortex_m;
use cortex_m_rt::entry;
use stm32f1xx_hal as hal;
use embedded_hal::digital::v2::OutputPin;

use ssd1306::{prelude::*, Builder, I2CDIBuilder};

use stm32f1xx_hal::{
    spi::{Mode, Phase, Polarity, Spi,},
    i2c::{BlockingI2c, DutyCycle, Mode as OtherMode},
    prelude::*,
    stm32,
};

use ds1302::{DS1302, Hours, Clock, Calendar, Mode as ds1302_mode};

use embedded_graphics::{
    fonts::{
        Font6x12,
        Text,
    },
    pixelcolor::BinaryColor,
    prelude::*,
    style::{
        TextStyle,
        PrimitiveStyleBuilder,
    },
};

use core::fmt::Write;
use heapless::String;
use heapless::consts::*;

#[entry]
fn main() -> ! {

    let dp = stm32::Peripherals::take().unwrap();
    let cp = cortex_m::peripheral::Peripherals::take().unwrap();
 
    let mut flash = dp.FLASH.constrain();
    let mut rcc = dp.RCC.constrain();

    let clocks = rcc.cfgr.sysclk(16.mhz())
    .pclk1(8.mhz())
    .freeze(&mut flash.acr);

    let mut delay = stm32f1xx_hal::delay::Delay::new(cp.SYST, clocks);
    let mut afio = dp.AFIO.constrain(&mut rcc.apb2);

    //ds1302 rtc
    let mut gpioa = dp.GPIOA.split(&mut rcc.apb2);
    let cs = gpioa.pa4.into_push_pull_output(&mut gpioa.crl);
    let sck = gpioa.pa5.into_alternate_push_pull(&mut gpioa.crl);
    let miso = gpioa.pa6.into_floating_input(&mut gpioa.crl);
    let mosi = gpioa.pa7.into_alternate_push_pull(&mut gpioa.crl);

    //ssd1307 oled
    let mut gpiob = dp.GPIOB.split(&mut rcc.apb2);
    let scl = gpiob.pb6.into_alternate_open_drain(&mut gpiob.crl);
    let sda = gpiob.pb7.into_alternate_open_drain(&mut gpiob.crl);

    let i2c = BlockingI2c::i2c1(
        dp.I2C1,
        (scl, sda),
        &mut afio.mapr,
        OtherMode::Fast {
            frequency: 400_000.hz(),
            duty_cycle: DutyCycle::Ratio2to1,
        },
        clocks,
        &mut rcc.apb1,
        1000,
        10,
        1000,
        1000,
    );

    let interface = I2CDIBuilder::new().init(i2c);

    let mut disp: GraphicsMode<_,_> = Builder::new()
    .size(DisplaySize128x32)
    .connect(interface).into();
    disp.init().unwrap();

    let spi_mode = Mode {
        polarity: Polarity::IdleLow,
        phase: Phase::CaptureOnFirstTransition,
    };
    let spi = Spi::spi1(
        dp.SPI1,
        (sck, miso, mosi),
        &mut afio.mapr,
        spi_mode,
        500.khz(),
        clocks,
        &mut rcc.apb2,
    );

    let mut ds1302 = DS1302::new(spi, cs, ds1302_mode::Hour12).unwrap();
   /* let h = Hours {hours: 4, am_pm: 1};
    let clk = Clock {
        hours: h,
        minutes: 29,
        seconds: 0
    };
    let cal = Calendar {
        day: 2,
        date: 27,
        month: 10,
        year: 2020
    };
    ds1302.set_clock_calendar(clk, cal).unwrap();
    ds1302.set_clock_mode(ds1302_mode::Hour24).unwrap(); */


    let mut data = String::<U32>::from(" ");
    let mut text = " ";
    loop {
        let cl = ds1302.get_clock_calendar().unwrap();
        match ds1302.mode {
            ds1302_mode::Hour12 => {
                if cl.0.hours.am_pm == 1 {text = " PM"}
                else {text = " AM"}
            }
            ds1302_mode::Hour24 => text = ""
        }
        let _=write!(data,"{} {}.{}.{}\n{:02}:{:02}:{:02} {}",
                    cl.1.day, cl.1.date, cl.1.month, cl.1.year,
                    cl.0.hours.hours, cl.0.minutes, cl.0.seconds, text);

        Text::new(data.as_str(), Point::new(30, 10))
        .into_styled(TextStyle::new(Font6x12, BinaryColor::On))
        .draw(&mut disp).unwrap();
            disp.flush().unwrap();
        disp.clear();
        data.clear();
    }
}

