#![no_main]
#![no_std]

use cortex_m;
use cortex_m_rt::entry;
use panic_halt as _;

use ssd1306::{prelude::*, Builder, I2CDIBuilder};

use stm32f1xx_hal::{
    i2c::{BlockingI2c, DutyCycle, Mode as OtherMode},
    prelude::*,
    spi::{Mode, Phase, Polarity, Spi},
    stm32,
    delay::Delay as HAL_DELAY
};

use ds1302::{Calendar, Clock, Delay, Hours, Mode as ds1302_mode, DS1302};

use embedded_graphics::{
    fonts::{Font6x12, Text},
    pixelcolor::BinaryColor,
    prelude::*,
    style::TextStyle,
};

use core::fmt::Write;
use heapless::consts::*;
use heapless::String;
use stm32f1xx_hal::spi::SpiBitFormat::LsbFirst;
use stm32f1xx_hal::timer::Timer;

struct MyClock<TIM, const TIMER_HZ: u32> {
    _timer: Timer<TIM>,
}

impl<TIM, const TIMER_HZ: u32> MyClock<TIM, TIMER_HZ> {
    fn new(timer: Timer<TIM>) -> Self {
        Self { _timer: timer }
    }
}

impl<TIM, const TIMER_HZ: u32> Delay<TIMER_HZ> for MyClock<TIM, TIMER_HZ> {
    type Error = core::convert::Infallible;

    fn now(&mut self) -> fugit::TimerInstantU32<TIMER_HZ> {
        fugit::TimerInstantU32::from_ticks(0)
    }

    fn start(&mut self, _duration: fugit::TimerDurationU32<TIMER_HZ>) -> Result<(), Self::Error> {
        Ok(())
    }

    fn wait(&mut self) -> nb::Result<(), Self::Error> {
        Ok(())
    }
}

#[entry]
fn main() -> ! {
    let dp = stm32::Peripherals::take().unwrap();
    let cp = cortex_m::peripheral::Peripherals::take().unwrap();

    let mut flash = dp.FLASH.constrain();
    let mut rcc = dp.RCC.constrain();

    let clocks = rcc
        .cfgr
        .sysclk(16.mhz())
        .pclk1(8.mhz())
        .freeze(&mut flash.acr);

    //let mut delay = stm32f1xx_hal::delay::Delay::new(cp.SYST, clocks);
    let mut afio = dp.AFIO.constrain();

    let mut delay = HAL_DELAY::new(cp.SYST, clocks);
    //ds1302 rtc
    let mut gpioa = dp.GPIOA.split();
    let cs = gpioa.pa4.into_push_pull_output(&mut gpioa.crl);
    let sck = gpioa.pa5.into_alternate_push_pull(&mut gpioa.crl);
    let miso = gpioa.pa6.into_floating_input(&mut gpioa.crl);
    let mosi = gpioa.pa7.into_alternate_push_pull(&mut gpioa.crl);

    //ssd1307 oled
    let mut gpiob = dp.GPIOB.split();
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
        1000,
        10,
        1000,
        1000,
    );

    let interface = I2CDIBuilder::new().init(i2c);

    let mut disp: GraphicsMode<_, _> = Builder::new()
        .size(DisplaySize128x32)
        .connect(interface)
        .into();
    disp.init().unwrap();

    let spi_mode = Mode {
        polarity: Polarity::IdleLow,
        phase: Phase::CaptureOnFirstTransition,
    };
    let mut spi = Spi::spi1(
        dp.SPI1,
        (sck, miso, mosi),
        &mut afio.mapr,
        spi_mode,
        500.khz(),
        clocks,
    );

    let timer = Timer::tim1(dp.TIM1, &clocks);
    let ds_timer: MyClock<_, 100> = MyClock::new(timer);

    spi.bit_format(LsbFirst);

    let mut ds1302 = DS1302::new(spi, cs, ds1302_mode::Hour12, ds_timer).unwrap();

    let h = Hours {
        hours: 19,
        am_pm: 0,
    };
    let clk = Clock {
        hours: h,
        minutes: 24,
        seconds: 0,
    };
    let cal = Calendar {
        day: 5,
        date: 19,
        month: 11,
        year: 2021,
    };
    ds1302.set_clock_calendar(clk, cal).unwrap();
    ds1302.set_clock_mode(ds1302_mode::Hour24).unwrap();

    let mut data = String::<U32>::from(" ");
    let mut text = " ";
    loop {
        let cl = ds1302.get_clock_calendar().unwrap();
        match ds1302.mode {
            ds1302_mode::Hour12 => {
                if cl.0.hours.am_pm == 1 {
                    text = " PM"
                } else {
                    text = " AM"
                }
            }
            ds1302_mode::Hour24 => text = "",
        }
        let _ = write!(
            data,
            "{} {}.{}.{}\n{:02}:{:02}:{:02} {}",
            cl.1.day,
            cl.1.date,
            cl.1.month,
            cl.1.year,
            cl.0.hours.hours,
            cl.0.minutes,
            cl.0.seconds,
            text
        );

        Text::new(data.as_str(), Point::new(30, 10))
            .into_styled(TextStyle::new(Font6x12, BinaryColor::On))
            .draw(&mut disp)
            .unwrap();
        disp.flush().unwrap();
        disp.clear();
        data.clear();

        delay.delay_ms(1000_u16);
    }
}
