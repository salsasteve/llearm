#![no_std]
#![no_main]

use defmt::info;
use defmt_rtt as _;
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, signal::Signal};
use embassy_time::Duration;
use esp_backtrace as _;
use esp_hal::{
    gpio::{Input, Level, Output, Pull},
    interrupt::{software::SoftwareInterruptControl, Priority},
    timer::{timg::TimerGroup, AnyTimer},
    Cpu,
};
use esp_hal_embassy::InterruptExecutor;
use static_cell::StaticCell;
use embassy_executor::Spawner;


#[embassy_executor::task]
async fn control_led(
    mut led: Output<'static>,
    control: &'static Signal<CriticalSectionRawMutex, bool>,
) {
    info!("Starting control_led() on core {}", Cpu::current() as usize);
    loop {
        if control.wait().await {
            led.toggle();
        }
    }
}

#[embassy_executor::task]
async fn monitor_button(
    button: Input<'static>,
    led_ctrl: &'static Signal<CriticalSectionRawMutex, bool>,
) {
    let mut last_state = button.is_high();
    
    loop {
        embassy_time::Timer::after(Duration::from_millis(20)).await;
        let current_state = button.is_high();
      
        if last_state && !current_state {
            info!("Button was pressed, toggle LED");
            led_ctrl.signal(true);
        }
        last_state = current_state;
    }
}

#[esp_hal_embassy::main]
async fn main(_spawner: Spawner) {
    let peripherals = esp_hal::init(esp_hal::Config::default());

    let sw_ints = SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    let timer0: AnyTimer = timg0.timer0.into();
    esp_hal_embassy::init(timer0);

    static LED_CTRL: StaticCell<Signal<CriticalSectionRawMutex, bool>> = StaticCell::new();
    let led_ctrl_signal = &*LED_CTRL.init(Signal::new());

    let led = Output::new(peripherals.GPIO35, Level::Low);
    let button = Input::new(peripherals.GPIO21, Pull::Up);

    static EXECUTOR_CORE_0: StaticCell<InterruptExecutor<0>> = StaticCell::new();
    let executor_core0 = InterruptExecutor::new(sw_ints.software_interrupt0);
    let executor_core0 = EXECUTOR_CORE_0.init(executor_core0);

    let spawner = executor_core0.start(Priority::Priority1);
    spawner.spawn(monitor_button(button, led_ctrl_signal)).ok();
    spawner.spawn(control_led(led, led_ctrl_signal)).ok();

    loop {}
}