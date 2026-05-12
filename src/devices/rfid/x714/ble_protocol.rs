use super::x714::X714;

impl X714 {
    /// BLE connection loop placeholder.
    ///
    /// A full BLE implementation requires the `btleplug` crate and platform
    /// BlueZ/CoreBluetooth/WinRT support. The structure mirrors Python's
    /// `BLEProtocol.connect_ble()`:
    ///
    /// 1. Scan for a device whose name starts with `config.ble.name`.
    /// 2. Connect with `BleakClient` equivalent.
    /// 3. Enable notify on the TX characteristic.
    /// 4. In the notify callback, call `on_receive(data)`.
    /// 5. In the maintenance loop, send `#ping` every 5 s.
    /// 6. On disconnect, loop back to step 1.
    ///
    /// To implement add `btleplug = "0.11"` to Cargo.toml and replace this body.
    pub(crate) async fn run_ble_loop(&self) {
        eprintln!(
            "[{}] BLE transport not yet implemented. \
             Add `btleplug` to Cargo.toml and implement run_ble_loop().",
            self.config.name
        );
    }

    pub async fn write_ble(&self, _data: &[u8]) -> Result<(), String> {
        Err("BLE transport not yet implemented".to_string())
    }
}
