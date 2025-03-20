# ESP32-S3 Rainbow LED with Embassy and RTIC Priorities

This project demonstrates a simple rainbow effect on a WS2812B LED (SmartLED) connected to an ESP32-S3, using the `esp-hal` and `embassy-rs` frameworks. It showcases the use of interrupt-driven async executors, RTIC priority levels, and peripheral sharing. The project also uses `defmt` for logging.

## Hardware Requirements

*   An ESP32-S3 development board (e.g., ESP32-S3-DevKitM-1).  It MUST have an LED connected to GPIO 48.
*   A WS2812B LED (NeoPixel, SmartLED) - tested with a single LED.  
*   Jumper wires to connect the LED to the ESP32-S3.

## Software Requirements

*   Rust toolchain with support for the `xtensa-esp32s3-none-elf` target.
*   `espup` for installing the necessary ESP-IDF components.
*   `probe-rs` for flashing the firmware.

## Installation and Setup

1.  **Install Rust:** Follow the instructions at [https://www.rust-lang.org/tools/install](https://www.rust-lang.org/tools/install).

2. **Install probe-rs** Follow the instructions at [https://probe.rs/docs/getting-started/installation/](https://probe.rs/docs/getting-started/installation/)
3. **Setup probe-rs** Follow the instructions at [https://probe.rs/docs/getting-started/probe-setup/](https://probe.rs/docs/getting-started/probe-setup/)

4.  **Install the `xtensa-esp32s3-none-elf` target:**

    ```bash
    rustup target add xtensa-esp32s3-none-elf
    ```

5.  **Install `espup`:**

    ```bash
    cargo install espup
    ```

6.  **Install the correct esp toolchain:**

    ```bash
    espup install --toolchain-version "1.84.0.0"
    ```

7.  **Install `cargo-espflash` and `espflash`:**

    ```bash
    cargo install cargo-espflash espflash
    ```

8. **Flash and Monitor**
    *  connect the ESP32-S3 to your computer

    ```bash
    cargo run --release
    ```
## Expected Output

*   The WS2812B LED should cycle through a rainbow of colors.
*   Should display log messages from the `defmt::info!` calls. You should see messages indicating the current color being set.
*   You'll observe that the `low_prio_async` task's messages are interrupted when the `low_prio_blocking` task is blocking, demonstrating priority-based preemption.

