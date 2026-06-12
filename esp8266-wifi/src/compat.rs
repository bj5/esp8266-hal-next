use core::ffi::c_void;
use core::cell::RefCell;
use critical_section::Mutex;
use linked_list_allocator::Heap;

// Private allocator for C-libraries compatibility heap
static COMPAT_HEAP: Mutex<RefCell<Heap>> = Mutex::new(RefCell::new(Heap::empty()));
static mut HEAP_MEM: [u8; 16384] = [0; 16384]; // 16KB dedicated heap for the closed-source driver

/// Initialize the compatibility allocator
pub fn init_compat_heap() {
    critical_section::with(|cs| {
        unsafe {
            COMPAT_HEAP.borrow(cs).borrow_mut().init(HEAP_MEM.as_mut_ptr(), HEAP_MEM.len());
        }
    });
}

// ==========================================
// C Memory Allocation Compatibility Layer
// ==========================================

#[no_mangle]
pub unsafe extern "C" fn malloc(size: usize) -> *mut u8 {
    use core::alloc::Layout;
    if size == 0 {
        return core::ptr::null_mut();
    }
    // Allocate extra 8 bytes to store the size of the block
    let layout = Layout::from_size_align(size + 8, 8).unwrap();
    critical_section::with(|cs| {
        let mut heap = COMPAT_HEAP.borrow(cs).borrow_mut();
        let ptr = heap.allocate_first_fit(layout);
        match ptr {
            Ok(non_null) => {
                let raw = non_null.as_ptr();
                *(raw as *mut usize) = size;
                raw.add(8)
            }
            Err(_) => core::ptr::null_mut(),
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn free(ptr: *mut u8) {
    use core::alloc::Layout;
    if ptr.is_null() {
        return;
    }
    let raw = ptr.sub(8);
    let size = *(raw as *mut usize);
    let layout = Layout::from_size_align(size + 8, 8).unwrap();
    critical_section::with(|cs| {
        let mut heap = COMPAT_HEAP.borrow(cs).borrow_mut();
        heap.deallocate(core::ptr::NonNull::new_unchecked(raw), layout);
    });
}

#[no_mangle]
pub unsafe extern "C" fn calloc(nmemb: usize, size: usize) -> *mut u8 {
    let total = nmemb * size;
    let ptr = malloc(total);
    if !ptr.is_null() {
        core::ptr::write_bytes(ptr, 0, total);
    }
    ptr
}

#[no_mangle]
pub unsafe extern "C" fn realloc(ptr: *mut u8, size: usize) -> *mut u8 {
    if ptr.is_null() {
        return malloc(size);
    }
    if size == 0 {
        free(ptr);
        return core::ptr::null_mut();
    }
    let raw = ptr.sub(8);
    let old_size = *(raw as *mut usize);
    let new_ptr = malloc(size);
    if !new_ptr.is_null() {
        let copy_size = core::cmp::min(old_size, size);
        core::ptr::copy_nonoverlapping(ptr, new_ptr, copy_size);
        free(ptr);
    }
    new_ptr
}

#[no_mangle]
pub unsafe extern "C" fn zalloc(size: usize) -> *mut u8 {
    calloc(1, size)
}

// ==========================================
// FreeRTOS Task and Queue Compatibility Layer
// ==========================================

#[no_mangle]
pub unsafe extern "C" fn xTaskCreate(
    _task_func: *const c_void,
    _name: *const u8,
    _stack_depth: u16,
    _param: *mut c_void,
    _ux_priority: u32,
    _task_handle: *mut *mut c_void,
) -> i32 {
    1 // pdPASS
}

#[no_mangle]
pub unsafe extern "C" fn vTaskDelay(_ticks: u32) {
    // Spin lock or nop for cooperative delay
}

#[no_mangle]
pub unsafe extern "C" fn xQueueGenericCreate(
    _queue_len: u32,
    _item_size: u32,
    _queue_type: u8,
) -> *mut c_void {
    malloc(8) as *mut c_void
}

#[no_mangle]
pub unsafe extern "C" fn xQueueGenericSend(
    _queue: *mut c_void,
    _item: *const c_void,
    _ticks_to_wait: u32,
    _copy_position: i32,
) -> i32 {
    1 // pdPASS
}

#[no_mangle]
pub unsafe extern "C" fn xQueueGenericReceive(
    _queue: *mut c_void,
    _buffer: *mut c_void,
    _ticks_to_wait: u32,
    _just_peeking: i32,
) -> i32 {
    1 // pdPASS
}

#[no_mangle]
pub unsafe extern "C" fn vQueueDelete(queue: *mut c_void) {
    free(queue as *mut u8);
}

// ==========================================
// Semaphore and Mutex Compatibility Layer
// ==========================================

#[no_mangle]
pub unsafe extern "C" fn xSemaphoreCreateBinary() -> *mut c_void {
    malloc(8) as *mut c_void
}

#[no_mangle]
pub unsafe extern "C" fn xSemaphoreCreateMutex() -> *mut c_void {
    malloc(8) as *mut c_void
}

#[no_mangle]
pub unsafe extern "C" fn xSemaphoreTake(_sem: *mut c_void, _ticks_to_wait: u32) -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "C" fn xSemaphoreGive(_sem: *mut c_void) -> i32 {
    1
}

// ==========================================
// Hardware Timers and Interrupts Compatibility
// ==========================================

#[no_mangle]
pub unsafe extern "C" fn vPortEnterCritical() {
    // Critical section placeholder
}

#[no_mangle]
pub unsafe extern "C" fn vPortExitCritical() {
    // Critical section placeholder
}

#[no_mangle]
pub unsafe extern "C" fn xTaskGetTickCount() -> u32 {
    let ticks: u32;
    core::arch::asm!("rsr.ccount {}", out(reg) ticks);
    ticks / 1000 // Convert to dummy tick count
}

#[no_mangle]
pub unsafe extern "C" fn system_get_time() -> u32 {
    let ticks: u32;
    core::arch::asm!("rsr.ccount {}", out(reg) ticks);
    ticks / 80 // ESP8266 CPU cycles to microseconds (assuming 80MHz)
}

// ==========================================
// Logging / Printf Compatibility Layer
// ==========================================

#[no_mangle]
pub unsafe extern "C" fn printf(_format: *const u8, ...) -> i32 {
    0
}

#[no_mangle]
pub unsafe extern "C" fn puts(_s: *const u8) -> i32 {
    0
}

#[no_mangle]
pub unsafe extern "C" fn putchar(_c: i32) -> i32 {
    0
}

// ==========================================
// Additional SDK / ROM Compatibility Stubs
// ==========================================

#[no_mangle]
pub unsafe extern "C" fn Cache_Read_Disable() {
    core::arch::asm!("movi a15, 0x400047f0", "jx a15", options(noreturn));
}

#[no_mangle]
pub unsafe extern "C" fn Cache_Read_Enable_New() {
    core::arch::asm!(
        "movi a2, 0",
        "movi a3, 0",
        "movi a4, 1",
        "movi a15, 0x40004678",
        "jx a15",
        options(noreturn)
    );
}

#[no_mangle]
pub unsafe extern "C" fn lldesc_build_chain() -> i32 {
    let ret: i32;
    core::arch::asm!(
        "movi a15, 0x40004f40",
        "callx0 a15",
        out("a2") ret
    );
    ret
}

#[no_mangle]
pub unsafe extern "C" fn PendSV() {}

#[no_mangle]
pub unsafe extern "C" fn Uart_Init() {}

#[no_mangle]
pub unsafe extern "C" fn uart_buff_switch() {}

#[no_mangle]
pub unsafe extern "C" fn __wifi_task_create(
    _task_func: *const c_void,
    _name: *const u8,
    _stack_depth: u16,
    _param: *mut c_void,
    _ux_priority: u32,
    _task_handle: *mut *mut c_void,
) -> i32 {
    1 // pdPASS
}

#[no_mangle]
pub unsafe extern "C" fn __wifi_task_get_max_priority() -> i32 {
    10
}

#[no_mangle]
pub unsafe extern "C" fn __wifi_task_resume_all() {}

#[no_mangle]
pub unsafe extern "C" fn __wifi_task_suspend_all() {}

#[no_mangle]
pub unsafe extern "C" fn __wifi_queue_create(len: u32, item_size: u32) -> *mut c_void {
    malloc((len * item_size) as usize + 8) as *mut c_void
}

#[no_mangle]
pub unsafe extern "C" fn __wifi_queue_recv(_queue: *mut c_void, _item: *mut c_void, _ticks: u32) -> i32 {
    1 // pdPASS
}

#[no_mangle]
pub unsafe extern "C" fn __wifi_queue_send(_queue: *mut c_void, _item: *const c_void, _ticks: u32) -> i32 {
    1 // pdPASS
}

#[no_mangle]
pub unsafe extern "C" fn __wifi_queue_msg_num(_queue: *mut c_void) -> i32 {
    0
}

#[no_mangle]
pub static mut _g_esp_wifi_ppt_task_stk_size: usize = 2048;

#[no_mangle]
pub unsafe extern "C" fn _heap_caps_free(ptr: *mut u8) {
    free(ptr);
}

#[no_mangle]
pub unsafe extern "C" fn _heap_caps_malloc(size: usize, _caps: u32) -> *mut u8 {
    malloc(size)
}

#[no_mangle]
pub unsafe extern "C" fn _heap_caps_zalloc(size: usize, _caps: u32) -> *mut u8 {
    zalloc(size)
}

#[no_mangle]
pub unsafe extern "C" fn esp_get_free_heap_size() -> u32 {
    32768
}

#[no_mangle]
pub unsafe extern "C" fn _xt_isr_mask(_mask: u32) {}

#[no_mangle]
pub unsafe extern "C" fn ccmp_decrypt() -> i32 { 0 }

#[no_mangle]
pub unsafe extern "C" fn ccmp_encrypt() -> i32 { 0 }

#[no_mangle]
pub unsafe extern "C" fn omac1_aes_128() -> i32 { 0 }

#[no_mangle]
pub unsafe extern "C" fn pbkdf2_sha1() -> i32 { 0 }

#[no_mangle]
pub unsafe extern "C" fn esp_event_send() -> i32 { 0 }

#[no_mangle]
pub unsafe extern "C" fn esp_log_early_timestamp() -> u32 {
    system_get_time()
}

#[no_mangle]
pub unsafe extern "C" fn esp_log_write(_level: i32, _tag: *const u8, _format: *const u8, ...) {}

#[no_mangle]
pub unsafe extern "C" fn ets_printf(_format: *const u8, ...) -> i32 {
    0
}

#[no_mangle]
pub unsafe extern "C" fn esp_phy_load_cal_and_init() {}

#[no_mangle]
pub unsafe extern "C" fn phy_open_rf() {}

#[no_mangle]
pub static mut g_misc_nvs: u32 = 0;

#[no_mangle]
pub unsafe extern "C" fn misc_nvs_init() -> i32 { 0 }

#[no_mangle]
pub unsafe extern "C" fn nvs_open(_name: *const u8, _mode: u32, _handle: *mut u32) -> i32 {
    0x1102 // ESP_ERR_NVS_NOT_FOUND
}

#[no_mangle]
pub unsafe extern "C" fn nvs_commit(_handle: u32) -> i32 { 0 }

#[no_mangle]
pub unsafe extern "C" fn nvs_get_u8(_handle: u32, _key: *const u8, _out: *mut u8) -> i32 { 0x1102 }

#[no_mangle]
pub unsafe extern "C" fn nvs_set_u8(_handle: u32, _key: *const u8, _val: u8) -> i32 { 0 }

#[no_mangle]
pub unsafe extern "C" fn nvs_get_u16(_handle: u32, _key: *const u8, _out: *mut u16) -> i32 { 0x1102 }

#[no_mangle]
pub unsafe extern "C" fn nvs_set_u16(_handle: u32, _key: *const u8, _val: u16) -> i32 { 0 }

#[no_mangle]
pub unsafe extern "C" fn nvs_get_i8(_handle: u32, _key: *const u8, _out: *mut i8) -> i32 { 0x1102 }

#[no_mangle]
pub unsafe extern "C" fn nvs_set_i8(_handle: u32, _key: *const u8, _val: i8) -> i32 { 0 }

#[no_mangle]
pub unsafe extern "C" fn nvs_get_blob(_handle: u32, _key: *const u8, _out: *mut c_void, _len: *mut usize) -> i32 { 0x1102 }

#[no_mangle]
pub unsafe extern "C" fn nvs_set_blob(_handle: u32, _key: *const u8, _val: *const c_void, _len: usize) -> i32 { 0 }

#[no_mangle]
pub unsafe extern "C" fn esp_random() -> u32 {
    core::ptr::read_volatile(0x3ff2_0e44 as *const u32)
}

#[no_mangle]
pub unsafe extern "C" fn os_get_random(buf: *mut u8, len: usize) -> i32 {
    let mut i = 0;
    while i < len {
        let val = esp_random();
        let bytes = val.to_ne_bytes();
        let copy_len = core::cmp::min(len - i, 4);
        core::ptr::copy_nonoverlapping(bytes.as_ptr(), buf.add(i), copy_len);
        i += copy_len;
    }
    0
}

#[no_mangle]
pub unsafe extern "C" fn esp_set_cpu_freq(_freq: u32) {}

#[no_mangle]
pub unsafe extern "C" fn ets_update_cpu_frequency(_freq: u32) {}

#[no_mangle]
pub unsafe extern "C" fn esp_sleep_lock() {}

#[no_mangle]
pub unsafe extern "C" fn esp_sleep_unlock() {}

#[no_mangle]
pub unsafe extern "C" fn esp_timer_get_time() -> i64 {
    system_get_time() as i64
}

#[no_mangle]
pub unsafe extern "C" fn ets_delay_us(us: u32) {
    let start: u32;
    core::arch::asm!("rsr.ccount {}", out(reg) start);
    let ticks = us * 80; // Assuming 80MHz
    loop {
        let current: u32;
        core::arch::asm!("rsr.ccount {}", out(reg) current);
        if current.wrapping_sub(start) >= ticks {
            break;
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn os_delay_us(us: u32) {
    ets_delay_us(us);
}

#[no_mangle]
pub unsafe extern "C" fn os_timer_arm(_timer: *mut c_void, _time: u32, _repeat: bool) {}

#[no_mangle]
pub unsafe extern "C" fn os_timer_arm_us(_timer: *mut c_void, _time: u32, _repeat: bool) {}

#[no_mangle]
pub unsafe extern "C" fn os_timer_disarm(_timer: *mut c_void) {}

#[no_mangle]
pub unsafe extern "C" fn os_timer_setfn(_timer: *mut c_void, _func: *const c_void, _arg: *mut c_void) {}

#[no_mangle]
pub unsafe extern "C" fn wifi_timer_init() -> i32 { 0 }

#[no_mangle]
pub unsafe extern "C" fn ets_timer_proc() {}

#[no_mangle]
pub unsafe extern "C" fn sprintf(_str: *mut u8, _format: *const u8, ...) -> i32 {
    0
}

#[no_mangle]
pub unsafe extern "C" fn strncmp(s1: *const u8, s2: *const u8, n: usize) -> i32 {
    for i in 0..n {
        let c1 = *s1.add(i);
        let c2 = *s2.add(i);
        if c1 != c2 {
            return (c1 as i32) - (c2 as i32);
        }
        if c1 == 0 {
            break;
        }
    }
    0
}

#[no_mangle]
pub unsafe extern "C" fn strncpy(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    let mut i = 0;
    while i < n {
        let c = *src.add(i);
        *dest.add(i) = c;
        if c == 0 {
            break;
        }
        i += 1;
    }
    while i < n {
        *dest.add(i) = 0;
        i += 1;
    }
    dest
}

#[no_mangle]
pub unsafe extern "C" fn gpio_output_set(
    set_mask: u32,
    clear_mask: u32,
    enable_mask: u32,
    disable_mask: u32,
) {
    if set_mask != 0 {
        core::ptr::write_volatile(0x6000_0304 as *mut u32, set_mask);
    }
    if clear_mask != 0 {
        core::ptr::write_volatile(0x6000_0308 as *mut u32, clear_mask);
    }
    if enable_mask != 0 {
        core::ptr::write_volatile(0x6000_0310 as *mut u32, enable_mask);
    }
    if disable_mask != 0 {
        core::ptr::write_volatile(0x6000_0314 as *mut u32, disable_mask);
    }
}

#[no_mangle]
pub unsafe extern "C" fn esp_wifi_try_rate_from_high() -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn vPortETSIntrLock() {}

#[no_mangle]
pub unsafe extern "C" fn vPortETSIntrUnlock() {}

