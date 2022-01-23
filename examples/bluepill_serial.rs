#![no_main]
#![no_std]

use cortex_m;
use cortex_m_rt::entry;
//use panic_halt as _;
use panic_probe as _;

use stm32f1xx_hal::{
    delay::Delay as HAL_DELAY,
    i2c::{BlockingI2c, DutyCycle, Mode as OtherMode},
    prelude::*,
    serial::{Config, Serial},
    spi::{Mode, Phase, Polarity, Spi},
    stm32,
};

use ds1302::{Calendar, Clock, Delay, Hours, Mode as ds1302_mode, DS1302};

use core::fmt::Write;
use nb::block;
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

    fn cancel(&mut self) -> Result<(), Self::Error> {
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

    let tx = gpioa.pa2.into_alternate_push_pull(&mut gpioa.crl);
    let rx = gpioa.pa3;

    let mut serial = Serial::usart2(
        dp.USART2,
        (tx, rx),
        &mut afio.mapr,
        Config::default().baudrate(9600.bps()),
        clocks,
    );
    let (mut tx, _rx) = serial.split();

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

    let h = Hours::Hour24(19);
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

    loop {
        let cl = ds1302.get_clock_calendar().unwrap();
        let (text, h) = match cl.0.hours {
            Hours::Hour24(h) => ("", h),
            Hours::Hour12am(h) => ("am", h),
            Hours::Hour12pm(h) => ("pm", h),
        };

        writeln!(
            tx,
            "{} {}.{}.{} {:02}:{:02}:{:02} {}",
            cl.1.day, cl.1.date, cl.1.month, cl.1.year, h, cl.0.minutes, cl.0.seconds, text
        );

        delay.delay_ms(1000_u16);
    }
}
