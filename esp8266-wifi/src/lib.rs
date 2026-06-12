#![no_std]
#![feature(c_variadic)]
#![feature(asm_experimental_arch)]

pub mod compat;
pub mod phy;

use core::ffi::c_void;

// C structs matching the SDK expectations
#[repr(C)]
pub struct WifiInitConfig {
    pub event_handler: Option<unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32>,
    pub osi_funcs: *mut c_void,
    pub wpa_crypto_funcs: *mut c_void,
    pub static_rx_buf_num: u32,
    pub dynamic_rx_buf_num: u32,
    pub tx_buf_num: u32,
    pub ampdu_rx_enable: u32,
    pub rx_ampdu_buf_len: u32,
    pub rx_ampdu_buf_num: u32,
    pub magic: u32,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct WifiConfigAp {
    pub ssid: [u8; 32],
    pub password: [u8; 64],
    pub ssid_len: u8,
    pub channel: u8,
    pub authmode: u32,
    pub ssid_hidden: u8,
    pub max_connection: u8,
    pub beacon_interval: u16,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct WifiConfigSta {
    pub ssid: [u8; 32],
    pub password: [u8; 64],
    pub scan_method: u32,
    pub bssid_set: u8,
    pub bssid: [u8; 6],
    pub channel: u8,
    pub listen_interval: u16,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub union WifiConfigUnion {
    pub ap: WifiConfigAp,
    pub sta: WifiConfigSta,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct WifiConfig {
    pub mode: u32,
    pub config: WifiConfigUnion,
}

// Bindings to the closed-source functions inside libnet80211.a, libpp.a, libphy.a
extern "C" {
    pub fn esp_wifi_deinit_internal() -> i32;
    pub fn esp_wifi_set_mode(mode: u32) -> i32;
    pub fn esp_wifi_get_mode(mode: *mut u32) -> i32;
    pub fn esp_wifi_set_config(interface: u32, config: *const WifiConfig) -> i32;
    pub fn esp_wifi_start() -> i32;
    pub fn esp_wifi_stop() -> i32;
    pub fn esp_wifi_connect() -> i32;
    pub fn esp_wifi_disconnect() -> i32;
    
    // PHY calibration and configuration APIs
    pub fn register_chipv6_phy();
    pub fn register_phy_ops();
    pub fn phy_get_romfunc_addr() -> *mut c_void;

    pub fn esp_wifi_set_rx_pbuf_mem_type(mem_type: i32) -> i32;
    pub fn esp_wifi_init_internal(config: *const WifiInitConfig) -> i32;
}

// Implement esp_wifi_init and esp_wifi_deinit in Rust to replace the SDK's C implementations
#[no_mangle]
pub unsafe extern "C" fn esp_wifi_init(config: *const WifiInitConfig) -> i32 {
    register_chipv6_phy();
    register_phy_ops();
    
    // We stub mac_init below in case it's not present in the precompiled libraries.
    mac_init();
    
    esp_wifi_set_rx_pbuf_mem_type(0); // WIFI_RX_PBUF_DRAM

    esp_wifi_init_internal(config)
}

#[no_mangle]
pub unsafe extern "C" fn esp_wifi_deinit() -> i32 {
    esp_wifi_deinit_internal()
}

#[no_mangle]
pub unsafe extern "C" fn mac_init() -> i32 {
    0
}

/// A handle to the ESP8266 WiFi driver
pub struct EspWifi {
    _private: (),
}

impl EspWifi {
    /// Initialize the WiFi controller
    pub fn init() -> Result<Self, i32> {
        // Initialize the OS compatibility layer and its heap allocator
        compat::init_compat_heap();

        unsafe {
            // Build initialization config
            let config = WifiInitConfig {
                event_handler: None,
                osi_funcs: core::ptr::null_mut(),
                wpa_crypto_funcs: core::ptr::null_mut(),
                static_rx_buf_num: 4,
                dynamic_rx_buf_num: 8,
                tx_buf_num: 4,
                ampdu_rx_enable: 0,
                rx_ampdu_buf_len: 0,
                rx_ampdu_buf_num: 0,
                magic: 0x5F54494E, // WiFi magic number
            };

            let err = esp_wifi_init(&config);
            if err != 0 {
                return Err(err);
            }
        }

        Ok(EspWifi { _private: () })
    }

    /// Set mode (Station = 1, SoftAP = 2, Station+SoftAP = 3)
    pub fn set_mode(&self, mode: u32) -> Result<(), i32> {
        unsafe {
            let err = esp_wifi_set_mode(mode);
            if err != 0 {
                return Err(err);
            }
        }
        Ok(())
    }

    /// Start WiFi driver (enables radio)
    pub fn start(&self) -> Result<(), i32> {
        unsafe {
            let err = esp_wifi_start();
            if err != 0 {
                return Err(err);
            }
        }
        Ok(())
    }

    /// Connect to access point (must be in station mode)
    pub fn connect(&self) -> Result<(), i32> {
        unsafe {
            let err = esp_wifi_connect();
            if err != 0 {
                return Err(err);
            }
        }
        Ok(())
    }
}
