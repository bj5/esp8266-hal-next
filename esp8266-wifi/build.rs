use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_dir = Path::new(&out_dir).join("esp8266-libs");
    fs::create_dir_all(&dest_dir).unwrap();

    // The list of closed-source WiFi libraries from the official ESP8266 RTOS SDK
    let libs = [
        "libphy.a",
        "libpp.a",
        "libnet80211.a",
    ];

    let base_url = "https://raw.githubusercontent.com/espressif/ESP8266_RTOS_SDK/master/components/esp8266/lib";

    for lib in &libs {
        let dest_path = dest_dir.join(lib);
        if !dest_path.exists() {
            println!("Downloading {} to {:?}", lib, dest_path);
            let url = format!("{}/{}", base_url, lib);
            let status = Command::new("curl")
                .args(["-L", "-s", "-o", dest_path.to_str().unwrap(), &url])
                .status()
                .expect("Failed to run curl");
            
            if !status.success() {
                panic!("Failed to download {} from {}", lib, url);
            }
        }
    }

    // Pass the library path to the linker
    println!("cargo:rustc-link-search=native={}", dest_dir.to_str().unwrap());

    // Link the libraries
    println!("cargo:rustc-link-lib=static=phy");
    println!("cargo:rustc-link-lib=static=pp");
    println!("cargo:rustc-link-lib=static=net80211");

    // Force rebuild if build.rs changes
    println!("cargo:rerun-if-changed=build.rs");
}
