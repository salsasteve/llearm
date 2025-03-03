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
    gpio::AnyPin,
    interrupt::{software::SoftwareInterruptControl, Priority},
    peripherals::RMT,
    prelude::*,
    rmt::Rmt,
    timer::{timg::TimerGroup, AnyTimer},
};
use esp_hal_embassy::InterruptExecutor;
use esp_hal_smartled::{smartLedBuffer, SmartLedsAdapter};
use smart_leds::{
    brightness, gamma, hsv::{hsv2rgb, Hsv}, SmartLedsWrite
};
use static_cell::StaticCell;

fn hue_to_color_name(hue: u8) -> &'static str {
    match hue {
        0..=10 => "red",
        11..=40 => "orange",
        41..=70 => "yellow",
        71..=100 => "green",
        101..=130 => "cyan",
        131..=160 => "blue",
        161..=190 => "purple",
        191..=220 => "magenta",
        221..=255 => "red", 
    }
}

#[embassy_executor::task]
async fn high_prio(led: AnyPin, peripheral_rmt: RMT) {
    info!("Starting high_prio()");
    let mut ticker = Ticker::every(Duration::from_secs(1));
    let rmt = Rmt::new(peripheral_rmt, 80.MHz()).unwrap();

    let rmt_buffer = smartLedBuffer!(1);
    let mut led = SmartLedsAdapter::new(rmt.channel0, led, rmt_buffer);

    let mut color = Hsv {
        hue: 0,
        sat: 255,
        val: 255,
    };
    let mut data;
    loop {
        // Iterate over the rainbow!
        for hue in (0..=255).step_by(4){
            color.hue = hue;
            data = [hsv2rgb(color)];
            led.write(brightness(gamma(data.iter().cloned()), 50))
                .unwrap();
            let color_name = hue_to_color_name(hue);
            info!("Setting color to {}", color_name);
            ticker.next().await;
        }
    }
}

#[embassy_executor::task]
async fn low_prio_blocking() {
    info!("Starting low-priority task that isn't actually async");
    loop {
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(5) {}
        Timer::after(Duration::from_secs(5)).await;
    }
}

/// A well-behaved, but starved async task.
#[embassy_executor::task]
async fn low_prio_async() {
    info!("Starting low-priority task that will not be able to run while the blocking task is running");
    let mut ticker = Ticker::every(Duration::from_secs(1));
    loop {
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
    let rgb_48 = peripherals.GPIO48;
    let rmt_pref = peripherals.RMT;

    info!("Spawning low-priority tasks");
    low_prio_spawner.must_spawn(low_prio_async());
    low_prio_spawner.must_spawn(low_prio_blocking());

    spawner.must_spawn(high_prio(rgb_48.into(), rmt_pref));

    spawner.spawn(low_prio_async()).ok();

    loop {
        Timer::after(Duration::from_millis(20)).await;
    }
}
