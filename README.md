# ESP8266 Rust HAL & WiFi Experimental Support Project

This repository provides a bare-metal Hardware Abstraction Layer (HAL) for the **ESP8266** microcontroller in Rust, along with an experimental sub-crate `esp8266-wifi` to integrate Espressif's official closed-source WiFi protocol stack.

Since the ESP8266 (Xtensa LX106 architecture) is deprecated and no longer supported by Espressif's modern Rust HAL crates (such as `esp-hal` and `esp-wifi`), this project bridges Espressif's official closed-source C static libraries from `ESP8266_RTOS_SDK` using Rust FFI. This enables building a lightweight, bare-metal (`no_std`) WiFi support library.

---

## 📂 Project Structure

This workspace is organized as a Cargo Workspace and contains the following components:

*   **[esp8266](file:///Users/jerryliu/Desktop/esp8266-hal/esp8266)**: Automatically generated Peripheral Access Crate (PAC).
*   **[esp8266-hal](file:///Users/jerryliu/Desktop/esp8266-hal)**: The core Hardware Abstraction Layer (providing drivers for GPIO, Timer, UART, Flash, Watchdog, etc.).
*   **[esp8266-wifi](file:///Users/jerryliu/Desktop/esp8266-hal/esp8266-wifi)**: The experimental WiFi protocol stack bridge library.
    *   **[build.rs](file:///Users/jerryliu/Desktop/esp8266-hal/esp8266-wifi/build.rs)**: Automatically downloads the required static libraries from GitHub.
    *   **[src/lib.rs](file:///Users/jerryliu/Desktop/esp8266-hal/esp8266-wifi/src/lib.rs)**: Declares the FFI bindings and implements the main `EspWifi` manager.
    *   **[src/compat.rs](file:///Users/jerryliu/Desktop/esp8266-hal/esp8266-wifi/src/compat.rs)**: Contains the C runtime compatibility layer and FreeRTOS API stubs.
    *   **[src/phy.rs](file:///Users/jerryliu/Desktop/esp8266-hal/esp8266-wifi/src/phy.rs)**: Connects the WiFi hardware radio to `smoltcp` via a custom physical device driver.
*   **[examples](file:///Users/jerryliu/Desktop/esp8266-hal/examples)**: A folder containing examples for basic peripherals and WiFi connectivity testing.

---

## 🛠️ Core Architecture & WiFi Implementation

### 1. Bridging Closed-Source Static Libraries (Blobs)
The RF control and 802.11 MAC protocol stack on the ESP8266 are encapsulated in Espressif's closed-source static libraries. During the build process, `build.rs` automatically downloads and links the following three libraries:
*   `libphy.a`: Physical RF calibration and driver.
*   `libpp.a`: 802.11 MAC layer protocol processor.
*   `libnet80211.a`: WiFi state machine and link manager.

### 2. C Runtime Compatibility Layer (OS Shim)
The closed-source libraries were originally developed for FreeRTOS and rely on standard C library functions. To run them in a bare-metal Rust environment, we implemented a compatibility layer in `compat.rs` using `#[no_mangle] extern "C"`:
*   **Dedicated Heap Allocator**: Allocates a 16KB static memory buffer (`HEAP_MEM`) managed by `linked_list_allocator::Heap` and protected by `critical-section`. It exports standard C functions like `malloc`, `free`, `calloc`, `realloc`, and `zalloc`.
*   **FreeRTOS System Calls**: Stubs task management (`__wifi_task_create`, etc.) and message queue (`__wifi_queue_create` / `__wifi_queue_recv`, etc.) APIs.
*   **Clock and Random Number Generators**: Implements high-precision delay (`ets_delay_us`) and system uptime (`system_get_time`) using the CPU cycles counter (`ccount`). A hardware random number generator is implemented by directly reading from the `0x3ff20e44` register.

### 3. ROM Function Offloading & DMA Redirecting
Since we do not link the SDK's ROM linker script directly, several functions embedded in the ESP8266 ROM produce `undefined reference` errors. We resolve this by using assembly tail-calls in `compat.rs` to redirect execution to actual hardware ROM addresses:
*   `Cache_Read_Disable` -> Jump to `0x400047f0` (disables instructions flash cache).
*   `Cache_Read_Enable_New` -> Jumps to `0x40004678` after setting parameters (enables and initializes instructions flash cache).
*   `lldesc_build_chain` -> Calls `0x40004f40` via assembly `callx0` (builds DMA descriptor chain).

### 4. Critical Section
The Xtensa LX106 core does not support hardware Compare-And-Swap (CAS) instructions. Consequently, synchronization is achieved by globally disabling interrupts. In `esp8266-hal/src/lib.rs`, we implement the `critical-section` backend:
*   **Acquire**: Uses `rsil {reg}, 15` to elevate interrupt priority to the maximum (disabling interrupts) and returns the previous state.
*   **Release**: Uses `wsr {reg}, ps` to restore the processor status register (PS), re-enabling interrupts.

### 5. Smoltcp Integration
In `phy.rs`, we implement `smoltcp::phy::Device` to map the low-level WiFi transmit callback (`esp_wifi_internal_tx`) and receive packet interrupts to Rust's pure-Rust TCP/IP network protocol stack (`smoltcp`), providing a full TCP/IP stack in a bare-metal context.

---

## 🚫 Compilation Workarounds

During development, we resolved several compiler and linker issues inherent to this architecture:

### 1. LLVM Xtensa Register Allocation Crash
*   **Symptom**: In Debug mode or when `debug-assertions` are enabled, LLVM crashes during register allocation for 64-bit float operations or soft-float emulation (`rustc-LLVM ERROR: Cannot scavenge register without an emergency spill slot!`).
*   **Workaround**: We configured a global override in [Cargo.toml](file:///Users/jerryliu/Desktop/esp8266-hal/Cargo.toml) to build all package dependencies and standard library components (like `compiler_builtins`) with code size optimization (`opt-level = "s"`) and disabled debug assertions/overflow checks:
    ```toml
    [profile.dev.package."*"]
    opt-level = "s"
    debug-assertions = false
    overflow-checks = false
    ```

### 2. Modern Toolchain Endianness Conflicts
*   **Symptom**: The latest Espressif toolchains integrate all Xtensa cores under a single driver `xtensa-esp-elf-gcc`. This driver defaults to Big-Endian, causing link failures for Rust's target files which are compiled as Little-Endian.
*   **Workaround**: In [`.cargo/config`](file:///Users/jerryliu/Desktop/esp8266-hal/.cargo/config), we configured the target `xtensa-esp8266-none-elf` to explicitly use the ESP32 linker `xtensa-esp32-elf-gcc` (which is Little-Endian compatible by default) and passed `-C link-arg=-mabi=call0` via `rustflags` to force the ESP8266-specific `call0` ABI.

---

## 🚀 Build & Run Guide

### 1. Toolchain Setup
To build the project, you must set up the Espressif Xtensa Rust compiler toolchain. Run the following commands to export the path variables (or save them to a script like `export-esp.sh` and run `source export-esp.sh`):

```bash
export PATH="/Users/jerryliu/.rustup/toolchains/esp/xtensa-esp-elf/esp-14.2.0_20240906/xtensa-esp-elf/bin:$PATH"
export LIBCLANG_PATH="/Users/jerryliu/.rustup/toolchains/esp/xtensa-esp32-elf-clang/esp-19.1.2_20250225/esp-clang/lib"
```

### 2. Building Examples
Since standard unit tests cannot run on the bare-metal target, you can verify build success by compiling the examples folder:

```bash
# Compile blinky and wifi_test examples
cargo +esp build --examples
```
Upon a successful build, the target static ELF binaries will be generated under `target/xtensa-esp8266-none-elf/debug/examples/`.

### 3. Flashing Firmware
We recommend using Espressif's official Rust flashing tool `espflash`:

```bash
# Install espflash
cargo install espflash

# Flash the blinky example (replace /dev/ttyUSB0 with your actual serial port)
espflash flash target/xtensa-esp8266-none-elf/debug/examples/blinky --monitor
```

---

## 📝 License

This project is dual-licensed under either:

*   Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
*   MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
