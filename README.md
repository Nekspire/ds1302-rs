# ds1302-rs

[![crates.io badge](https://img.shields.io/crates/v/ds1302.svg)](https://crates.io/crates/ds1302)
[![docs.rs badge](https://docs.rs/ds1302/badge.svg)](https://docs.rs/ds1302)


 DS1302 real time clock-calendar platform agnostic driver

 # About
 
 DS1302 is a real time clock/calendar chip, which communicates via SPI interface. The device provides seconds, minutes, hours, day, date, month, and year information.
 The driver is based on [`embedded-hal`] traits.

Datasheet: [DS1302](https://datasheets.maximintegrated.com/en/ds/DS1302.pdf)

![](images/ds1302_board.jpg) 
                        
DS1302 RTC Board - Waveshare
 
 [`embedded-hal`]: https://github.com/rust-embedded/embedded-hal

 
 ## Hardware requirements
 - **Importand**: SPI frame format with **LSB transmitted first!**
 - SPI speed less than 2 MHz
 - CPOL: CK to 0 when idle, CPHA: the first clock transition is the first data capture edge
 - Default 8-bit data frame format is selected for transmission/reception

## Features:

- Reading/setting clock and calendar data in 12-hour or 24-hour format.
- Changing hour format without resetting it. `set_clock_mode()`

## TODO:

- RAM support.

## Examples
https://github.com/Nekspire/ds1302-rs/tree/master/examples

 To build examples run:

`cargo build --examples` or `cargo build --examples --release`

 ## License

Copyright © 2021 Nekspire

Dual licensed under your choice of either of:

- Apache License, Version 2.0, (LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license (LICENSE-MIT or http://opensource.org/licenses/MIT)


Thanks for contribution!