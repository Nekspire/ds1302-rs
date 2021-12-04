# ds1302-rs

[![crates.io badge](https://img.shields.io/crates/v/ds1302.svg)](https://crates.io/crates/ds1302)
[![docs.rs badge](https://docs.rs/ds1302/badge.svg)](https://docs.rs/ds1302)


 DS1302 real time clock-calendar platform agnostic driver

 # About

The DS1302 trickle-charge timekeeping chip contains a real-time clock/calendar and 31 bytes of static RAM. It
communicates with a microprocessor via a simple serial interface. The real-time clock/calendar provides seconds,
minutes, hours, day, date, month, and year information. The end of the month date is automatically adjusted for
months with fewer than 31 days, including corrections for leap year. The clock operates in either the 24-hour or
12-hour format with an AM/PM indicator. The chip driver is based on [`embedded-hal`] traits.

Datasheet: [DS1302](https://datasheets.maximintegrated.com/en/ds/DS1302.pdf)

![](images/ds1302_board.jpg) 
                        
DS1302 RTC Board - Waveshare
 
 [`embedded-hal`]: https://github.com/rust-embedded/embedded-hal

 
 ## Hardware requirements
- Serial Peripheral Interface (SPI)
- SPI speed **less than 2 MHz**
- SPI frame format with **LSB transmitted first!**
- Default **8-bit data frame** format is selected for transmission/reception
- Default CPOL: CK to 0 when idle, CPHA: the first clock transition is the first data capture edge

## Driver features:

- Reading/setting clock/calendar data 
- 12-hour (AM/PM) or 24-hour format
- Changing the time format while the chip is working


  NEW (4.0.0 release):
- Programmable Trickle Charger configuration
- 31 x 8 Battery-Backed General-Purpose RAM operations

## Examples
https://github.com/Nekspire/ds1302-rs/tree/master/examples

This crate uses [`probe-run`](https://crates.io/crates/probe-run) to run the examples.

To build examples type:

`cargo build --examples` or `cargo build --examples --release`

To run examples type:

`cargo run --example <example name>` or `cargo run --example <example name> --release`

The output should be like this:

```
Running `probe-run --chip STM32F103C8 target/thumbv7m-none-eabi/debug/examples/bluepill_ssd1306`
(HOST) INFO  flashing program (36.32 KiB)
(HOST) INFO  success!
```

 ## License

Copyright © 2021 Nekspire

Dual licensed under your choice of either of:

- Apache License, Version 2.0, (LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license (LICENSE-MIT or http://opensource.org/licenses/MIT)


Thanks for contribution!