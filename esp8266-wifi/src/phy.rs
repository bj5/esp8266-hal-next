use core::cell::RefCell;
use core::ffi::c_void;
use critical_section::Mutex;
use smoltcp::phy::{Device, DeviceCapabilities, RxToken, TxToken};
use smoltcp::time::Instant;

// Reference to typical internal WiFi TX/RX functions in the Espressif blobs
extern "C" {
    // Send a raw packet on a specific interface (0 = Station, 1 = AP)
    pub fn esp_wifi_internal_tx(ifx: u32, buffer: *const u8, len: u16) -> i32;
    // Register a callback for when a packet is received on a specific interface
    pub fn esp_wifi_internal_reg_rxcb(
        ifx: u32,
        cb: unsafe extern "C" fn(buffer: *mut u8, len: u16, eb: *mut c_void) -> i32,
    ) -> i32;
    // Free the Espressif-allocated RX packet buffer inside the callback
    pub fn esp_wifi_internal_free_rx_buffer(eb: *mut c_void);
}

// Simple circular queue for received packets to bridge interrupt callbacks to smoltcp
const RX_QUEUE_SIZE: usize = 4;
const MTU: usize = 1514;

struct RxPacket {
    len: usize,
    data: [u8; MTU],
}

struct RxQueue {
    packets: [RxPacket; RX_QUEUE_SIZE],
    read_idx: usize,
    write_idx: usize,
    count: usize,
}

static RX_QUEUE: Mutex<RefCell<RxQueue>> = Mutex::new(RefCell::new(RxQueue {
    packets: unsafe { core::mem::zeroed() },
    read_idx: 0,
    write_idx: 0,
    count: 0,
}));

// The FFI Rx callback invoked directly by the Espressif closed-source WiFi ISR
unsafe extern "C" fn wifi_rx_callback(buffer: *mut u8, len: u16, eb: *mut c_void) -> i32 {
    critical_section::with(|cs| {
        let mut queue = RX_QUEUE.borrow(cs).borrow_mut();
        if queue.count < RX_QUEUE_SIZE && (len as usize) <= MTU {
            let write_idx = queue.write_idx;
            let packet = &mut queue.packets[write_idx];
            packet.len = len as usize;
            core::ptr::copy_nonoverlapping(buffer, packet.data.as_mut_ptr(), len as usize);
            queue.write_idx = (write_idx + 1) % RX_QUEUE_SIZE;
            queue.count += 1;
        }
    });

    // Notify the Espressif stack that we've copied/handled the buffer, so it can free it
    esp_wifi_internal_free_rx_buffer(eb);
    0
}

pub struct WifiDevice {
    interface_id: u32, // 0 = Station, 1 = AP
}

impl WifiDevice {
    pub fn new(interface_id: u32) -> Self {
        unsafe {
            // Register our callback function for incoming network packets
            esp_wifi_internal_reg_rxcb(interface_id, wifi_rx_callback);
        }
        WifiDevice { interface_id }
    }
}

impl Device for WifiDevice {
    type RxToken<'a> = WifiRxToken where Self: 'a;
    type TxToken<'a> = WifiTxToken<'a> where Self: 'a;

    fn receive(&mut self, _timestamp: Instant) -> Option<(Self::RxToken<'_>, Self::TxToken<'_>)> {
        critical_section::with(|cs| {
            let queue = RX_QUEUE.borrow(cs).borrow_mut();
            if queue.count > 0 {
                let read_idx = queue.read_idx;
                let packet_len = queue.packets[read_idx].len;
                
                // Construct tokens
                let rx = WifiRxToken {
                    read_idx,
                    len: packet_len,
                };
                let tx = WifiTxToken {
                    device: self,
                };
                Some((rx, tx))
            } else {
                None
            }
        })
    }

    fn transmit(&mut self, _timestamp: Instant) -> Option<Self::TxToken<'_>> {
        Some(WifiTxToken { device: self })
    }

    fn capabilities(&self) -> DeviceCapabilities {
        let mut caps = DeviceCapabilities::default();
        caps.max_transmission_unit = MTU;
        caps.medium = smoltcp::phy::Medium::Ethernet;
        caps
    }
}

pub struct WifiRxToken {
    read_idx: usize,
    len: usize,
}

impl RxToken for WifiRxToken {
    fn consume<R, F>(self, f: F) -> R
    where
        F: FnOnce(&mut [u8]) -> R,
    {
        let result = critical_section::with(|cs| {
            let mut queue = RX_QUEUE.borrow(cs).borrow_mut();
            let packet = &mut queue.packets[self.read_idx];
            let res = f(&mut packet.data[..self.len]);
            
            // Advance the read pointer
            queue.read_idx = (self.read_idx + 1) % RX_QUEUE_SIZE;
            queue.count -= 1;
            res
        });
        result
    }
}

pub struct WifiTxToken<'a> {
    device: &'a mut WifiDevice,
}

impl<'a> TxToken for WifiTxToken<'a> {
    fn consume<R, F>(self, len: usize, f: F) -> R
    where
        F: FnOnce(&mut [u8]) -> R,
    {
        let mut buffer = [0u8; MTU];
        let res = f(&mut buffer[..len]);
        unsafe {
            // Forward the packet payload to the Espressif WiFi radio transmitting buffer
            esp_wifi_internal_tx(self.device.interface_id, buffer.as_ptr(), len as u16);
        }
        res
    }
}
