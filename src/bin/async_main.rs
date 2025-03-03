//! This example shows how to use the interrupt executors to prioritize some
//! tasks over others.
//!
//! The example creates three tasks:
//!  - A low priority task that is not actually async, but simulates some
//!    blocking work. This task will run for 5 seconds, then sleep for 5
//!    seconds.
//!  - A low priority task that is actually async, but will not be able to run
//!    while the blocking task is running.
//!  - A high priority task that prints something every second. The example
//!    demonstrates that this task will continue to run even while the low
//!    priority blocking task is running.

// The thread-executor is created by the `#[esp_hal_embassy::main]` macro and is used to spawn `low_prio_async` and `low_prio_blocking`.
// The interrupt-executor is created in `main` and is used to spawn `high_prio`.

//% CHIPS: esp32 esp32c2 esp32c3 esp32c6 esp32h2 esp32s2 esp32s3
//% FEATURES: embassy esp-hal-embassy/log esp-hal-embassy/integrated-timers

#![no_std]
#![no_main]

use defmt::info;
use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_time::{Duration, Instant, Ticker, Timer};
use esp_backtrace as _;
use esp_hal::{
    prelude::*,
    interrupt::{software::SoftwareInterruptControl, Priority},
    timer::{timg::TimerGroup, AnyTimer},
    rmt::Rmt,
};
use esp_hal_embassy::InterruptExecutor;
use static_cell::StaticCell;
use esp_hal_smartled::{smartLedBuffer, SmartLedsAdapter};
use smart_leds::{
    brightness,
    gamma,
    hsv::{hsv2rgb, Hsv},
    SmartLedsWrite,
};

/// Periodically print something.
#[embassy_executor::task]
async fn high_prio() {
    info!("Starting high_prio()");
    let mut ticker = Ticker::every(Duration::from_secs(1));
    loop {
        info!("High priority ticks");
        ticker.next().await;
    }
}

/// Simulates some blocking (badly behaving) task.
#[embassy_executor::task]
async fn low_prio_blocking() {
    info!("Starting low-priority task that isn't actually async");
    loop {
        info!("Doing some long and complicated calculation");
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(5) {}
        info!("Calculation finished");
        Timer::after(Duration::from_secs(5)).await;
    }
}

/// A well-behaved, but starved async task.
#[embassy_executor::task]
async fn low_prio_async() {
    info!("Starting low-priority task that will not be able to run while the blocking task is running");
    let mut ticker = Ticker::every(Duration::from_secs(1));
    loop {
        info!("Low priority ticks");
        ticker.next().await;
    }
}

#[esp_hal_embassy::main]
async fn main(low_prio_spawner: Spawner) {
    info!("Init!");

    let peripherals = esp_hal::init(esp_hal::Config::default());

    let sw_ints = SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    let timer0: AnyTimer = timg0.timer0.into();

    let timg1 = TimerGroup::new(peripherals.TIMG1);
    let timer1: AnyTimer = timg1.timer0.into();

    esp_hal_embassy::init([timer0, timer1]);

    static EXECUTOR: StaticCell<InterruptExecutor<2>> = StaticCell::new();
    let executor = InterruptExecutor::new(sw_ints.software_interrupt2);
    let executor = EXECUTOR.init(executor);

    let spawner = executor.start(Priority::Priority3);
    // spawner.must_spawn(high_prio());

    // info!("Spawning low-priority tasks");
    // low_prio_spawner.must_spawn(low_prio_async());
    // low_prio_spawner.must_spawn(low_prio_blocking());
    let led = peripherals.GPIO48;

    let rmt = Rmt::new(peripherals.RMT, 80.MHz()).unwrap();

    // We use one of the RMT channels to instantiate a `SmartLedsAdapter` which can
    // be used directly with all `smart_led` implementations
    let rmt_buffer = smartLedBuffer!(1);
    let mut led = SmartLedsAdapter::new(rmt.channel0, led, rmt_buffer);

    let mut color = Hsv {
        hue: 0,
        sat: 255,
        val: 255,
    };
    let mut data;

    spawner.spawn(low_prio_async()).ok();

    loop {
        // Iterate over the rainbow!
        for hue in 0..=255 {
            color.hue = hue;
            // Convert from the HSV color space (where we can easily transition from one
            // color to the other) to the RGB color space that we can then send to the LED
            data = [hsv2rgb(color)];
            // When sending to the LED, we do a gamma correction first (see smart_leds
            // documentation for details) and then limit the brightness to 10 out of 255 so
            // that the output it's not too bright.
            led.write(brightness(gamma(data.iter().cloned()), 10))
                .unwrap();
            Timer::after(Duration::from_millis(20)).await;
        }
    }
}
