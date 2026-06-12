# ESP8266 Rust HAL & WiFi 实验性支持项目

本仓库是为 **ESP8266** 微控制器提供 Rust 裸机驱动支持的硬件抽象层（HAL），并包含了一个实验性的、用于集成乐鑫官方闭源 WiFi 协议栈的子 Crate：`esp8266-wifi`。

由于 ESP8266 (Xtensa LX106 架构) 属于已过时的微控制器，乐鑫官方的现代 Rust 硬件抽象层（如 `esp-hal` 和 `esp-wifi`）已不再原生支持该芯片。本项目旨在通过 Rust FFI 桥接乐鑫官方 `ESP8266_RTOS_SDK` 中的闭源 C 语言静态库，并在无操作系统的纯裸机（`no_std`）环境下构建一个轻量级 WiFi 支持库。

---

## 📂 项目结构

工作区采用 Cargo Workspace 组织结构，包含以下核心部分：

*   **[esp8266](file:///Users/jerryliu/Desktop/esp8266/src)**: 自动生成的外设访问层（PAC, Peripheral Access Crate）。
*   **[esp8266-hal](file:///Users/jerryliu/Desktop/esp8266-hal/src)**: 核心硬件抽象层（提供 GPIO、Timer、UART、Flash、Watchdog 等驱动）。
*   **[esp8266-wifi](file:///Users/jerryliu/Desktop/esp8266-hal/esp8266-wifi)**: WiFi 协议栈桥接子库（实验性）。
    *   **[build.rs](file:///Users/jerryliu/Desktop/esp8266-hal/esp8266-wifi/build.rs)**: 自动从 GitHub 下载必需的静态链接库。
    *   **[src/lib.rs](file:///Users/jerryliu/Desktop/esp8266-hal/esp8266-wifi/src/lib.rs)**: FFI 接口声明及 WiFi 主控结构 `EspWifi` 实现。
    *   **[src/compat.rs](file:///Users/jerryliu/Desktop/esp8266-hal/esp8266-wifi/src/compat.rs)**: FreeRTOS 桩函数与 OS 兼容层。
    *   **[src/phy.rs](file:///Users/jerryliu/Desktop/esp8266-hal/esp8266-wifi/src/phy.rs)**: 对接 `smoltcp` 协议栈的网络物理层设备驱动。
*   **[examples](file:///Users/jerryliu/Desktop/esp8266-hal/examples)**: 示例工程目录，包含外设基础测试及 WiFi 连网测试。

---

## 🛠️ 核心架构与 WiFi 实现方案

### 1. 闭源静态库 (Blobs) 桥接
ESP8266 的射频控制与 802.11 协议栈封装在乐鑫官方的闭源静态库中。在 `esp8266-wifi` 编译时，`build.rs` 会自动拉取以下三个必需的官方静态链接库并链接到二进制中：
*   `libphy.a`: 射频（PHY）物理层校准及驱动。
*   `libpp.a`: 802.11 协议 MAC 层处理。
*   `libnet80211.a`: WiFi 控制状态机与链路管理。

### 2. C 运行期兼容层 (OS Shim)
闭源库最初基于 FreeRTOS 编写，并依赖大量标准 C 库函数。为了让它们能在裸机 Rust 环境中运行，我们在 `compat.rs` 中使用 `#[no_mangle] extern "C"` 实现了兼容层：
*   **独立堆内存分配器**：分配了 16KB 专用静态区 `HEAP_MEM`，并由 `linked_list_allocator::Heap` 和 `critical_section` 保护，导出标准 C 内存操作函数 `malloc`、`free`、`calloc`、`realloc`、`zalloc`。
*   **FreeRTOS 系统调用桥接**：桩化了任务调度器（`__wifi_task_create` 等）与消息队列（`__wifi_queue_create` / `__wifi_queue_recv` 等）API。
*   **随机数与时钟发生器**：利用 CPU 周期寄存器 `ccount` 实现了高精度延时 `ets_delay_us` 及时钟 `system_get_time`，直接读取硬件寄存器 `0x3ff20e44` 实现了硬件随机数发生器。

### 3. ROM 映射与 DMA 拦截
由于我们没有直接链接 SDK 的 ROM 链接脚本，部分内嵌在 ESP8266 硬件 ROM 中的函数会报 `undefined reference` 链接错误。我们采用内联汇编尾调用（Tail-call Jump）技术在 `compat.rs` 中将其硬编码重定向至真实的 ROM 地址：
*   `Cache_Read_Disable` -> `0x400047f0`（ROM 中禁用 flash 缓存指令）。
*   `Cache_Read_Enable_New` -> 构造参数后跳转至 `0x40004678`（ROM 中启用并初始化 flash 缓存）。
*   `lldesc_build_chain` -> 通过汇编 `callx0` 调用 `0x40004f40`（ROM 中链表 DMA 构造器）。

### 4. Critical Section 临界区锁
由于 Xtensa LX106 核心不支持 CAS（Compare-And-Swap）原子硬件指令，并发状态的同步需要通过全局禁用中断实现。我们在 HAL 库的 `src/lib.rs` 中使用 `critical-section` 规范：
*   **获取锁 (Acquire)**：利用 `rsil {reg}, 15` 将中断优先级临时提升至最大值（禁能中断），并返回先前的状态。
*   **释放锁 (Release)**：利用 `wsr {reg}, ps` 恢复原来的处理器状态（PS）以重新使能中断。

### 5. Smoltcp 协议栈集成
在 `phy.rs` 中，我们实现了 `smoltcp::phy::Device` 接口，将 WiFi 射频的底层发送回调 (`esp_wifi_internal_tx`) 以及 WiFi 接收数据帧中断回调与 Rust 社区的纯 Rust 网络协议栈 (`smoltcp`) 对接，从而在裸机下获得完整的 TCP/IP 运行期。

---

## 🚫 历史编译障碍与解决方案

在开发过程中，我们遇到了该架构固有的编译器缺陷与环境冲突，并在工作区中进行了相应规避：

### 1. LLVM Xtensa 后端寄存器分配崩溃
*   **错误表现**：在 Debug 编译模式或开启 `debug-assertions` 时，LLVM 生成 64 位浮点数或软浮点操作会遭遇寄存器溢出崩溃（`rustc-LLVM ERROR: Cannot scavenge register without an emergency spill slot!`）。
*   **解决方案**：我们在根目录 [Cargo.toml](file:///Users/jerryliu/Desktop/esp8266-hal/Cargo.toml) 中配置了全局依赖重写规则，强制所有第三方 crate 和标准库（`compiler_builtins` 等）在编译时使用 `opt-level = "s"` 并关闭调试断言和溢出检查：
    ```toml
    [profile.dev.package."*"]
    opt-level = "s"
    debug-assertions = false
    overflow-checks = false
    ```

### 2. 现代交叉编译链的端序 (Endianness) 冲突
*   **错误表现**：乐鑫最新版的 GCC 工具链将所有 Xtensa 架构整合在统一的 `xtensa-esp-elf-gcc` 驱动程序下，而该驱动默认编译目标为大端（Big-Endian），直接使用它做链接器会导致 Rust 生成的小端目标文件链接失败。
*   **解决方案**：我们在 [`.cargo/config`](file:///Users/jerryliu/Desktop/esp8266-hal/.cargo/config) 中精细配置了 `xtensa-esp8266-none-elf` 目标属性，将链接器强指定为针对 ESP32 的 `xtensa-esp32-elf-gcc`（默认为 Little-Endian 兼容），并通过 `rustflags` 传递 `-C link-arg=-mabi=call0` 以强制使用 ESP8266 所属的 `call0` ABI。

---

## 🚀 编译与运行指南

### 1. 编译环境配置
在运行编译前，需要配置好乐鑫的 Xtensa Rust 交叉编译链。你可以直接在终端中运行以下环境变量设置（或将其保存为 `export-esp.sh` 后执行 `source export-esp.sh`）：

```bash
export PATH="/Users/jerryliu/.rustup/toolchains/esp/xtensa-esp-elf/esp-14.2.0_20240906/xtensa-esp-elf/bin:$PATH"
export LIBCLANG_PATH="/Users/jerryliu/.rustup/toolchains/esp/xtensa-esp32-elf-clang/esp-19.1.2_20250225/esp-clang/lib"
```

### 2. 构建工程示例
由于 ESP8266 属于裸机目标，标准库的测试依赖单元无法编译，因此我们直接构建示例工程（`examples`）：

```bash
# 编译 blinky 与 wifi_test 示例
cargo +esp build --examples
```
编译成功后，将在 `target/xtensa-esp8266-none-elf/debug/examples/` 目录下生成可用的静态 ELF 固件。

### 3. 烧录固件
我们推荐使用乐鑫官方提供的 Rust 烧录工具 `espflash`：

```bash
# 安装 espflash
cargo install espflash

# 烧录 blinky 示例 (请将 /dev/ttyUSB0 替换为您的实际串口)
espflash flash target/xtensa-esp8266-none-elf/debug/examples/blinky --monitor
```

---

## 📝 许可证

本项目基于 MIT 或 Apache-2.0 许可证开放源代码。
