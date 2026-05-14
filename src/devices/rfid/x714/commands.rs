use tokio::time::{Duration, sleep};

use super::x714::X714;

impl X714 {
    // ─── Command builders ─────────────────────────────────────────────────────

    pub fn config_commands(&self) -> Vec<String> {
        let mut cmds = Vec::new();

        cmds.push(self.protected_inventory_command(self.config.protected_inventory_active, None));

        for (antenna, cfg) in &self.config.ant_dict {
            cmds.push(format!(
                "#set_ant:{},{},{},{}",
                antenna,
                if cfg.active { "on" } else { "off" },
                cfg.power,
                cfg.rssi.abs()
            ));
        }

        cmds.push(format!("#session:{}", self.config.session));
        cmds.push(format!(
            "#start_reading:{}",
            if self.config.start_reading {
                "on"
            } else {
                "off"
            }
        ));
        cmds.push(format!(
            "#gpi_start:{}",
            if self.config.gpi_start { "on" } else { "off" }
        ));
        cmds.push(format!(
            "#always_send:{}",
            if self.config.always_send { "on" } else { "off" }
        ));
        cmds.push(format!(
            "#simple_send:{}",
            if self.config.simple_send { "on" } else { "off" }
        ));
        cmds.push(format!(
            "#keyboard:{}",
            if self.config.keyboard { "on" } else { "off" }
        ));
        cmds.push(format!(
            "#buzzer:{}",
            if self.config.buzzer { "on" } else { "off" }
        ));
        cmds.push(format!(
            "#decode_gtin:{}",
            if self.config.decode_gtin { "on" } else { "off" }
        ));
        cmds.push(format!(
            "#hotspot:{}",
            if self.config.hotspot { "on" } else { "off" }
        ));
        cmds.push(format!("#prefix:{}", self.config.prefix));
        cmds.push("#setup_reader".to_string());
        cmds.push("#get_info".to_string());
        cmds
    }

    pub fn start_inventory_command(&self) -> Option<String> {
        if self.config.gpi_start {
            None
        } else {
            Some("#READ:ON".to_string())
        }
    }

    pub fn stop_inventory_command(&self) -> Option<String> {
        if self.config.gpi_start {
            None
        } else {
            Some("#READ:OFF".to_string())
        }
    }

    pub fn clear_tags_command(&self) -> String {
        "#CLEAR".to_string()
    }

    pub fn protected_inventory_command(&self, active: bool, password: Option<&str>) -> String {
        if active {
            let pwd = password.unwrap_or(&self.config.protected_inventory_password);
            format!("#protected_inventory:on;{}", pwd)
        } else {
            "#protected_inventory:off".to_string()
        }
    }

    pub fn protected_mode_command(
        &self,
        epc: &str,
        password: Option<&str>,
        active: bool,
    ) -> String {
        let pwd = password.unwrap_or(&self.config.protected_inventory_password);
        format!(
            "#protected_mode:{};{};{}",
            epc,
            pwd,
            if active { "on" } else { "off" }
        )
    }

    // ─── Async commands ───────────────────────────────────────────────────────

    pub async fn start_inventory(&self) -> Result<(), String> {
        if let Some(cmd) = self.start_inventory_command() {
            self.write(&cmd).await?;
            self.on_start();
        }
        Ok(())
    }

    pub async fn stop_inventory(&self) -> Result<(), String> {
        if let Some(cmd) = self.stop_inventory_command() {
            self.write(&cmd).await?;
            self.on_stop();
        }
        Ok(())
    }

    pub async fn clear_tags(&self) -> Result<(), String> {
        self.write(&self.clear_tags_command()).await
    }

    pub async fn config_reader(&self) -> Result<(), String> {
        // For BLE, throttling is handled by the 150 ms inter-write delay inside the BLE
        // write task (ble_protocol.rs). Commands are queued to the mpsc channel here and
        // processed sequentially by that task, so no extra delays are needed here.
        for cmd in self.config_commands() {
            self.write(&cmd).await?;
        }
        Ok(())
    }

    pub async fn get_reader_info(&self) -> Result<(), String> {
        while self.is_connected() && self.serial_number().is_none() {
            self.write("#get_info").await?;
            sleep(Duration::from_secs(1)).await;
        }
        Ok(())
    }

    pub async fn write_epc(
        &self,
        target_identifier: Option<&str>,
        target_value: Option<&str>,
        new_epc: &str,
        password: &str,
    ) -> Result<(), String> {
        let command = if let Some(identifier) = target_identifier {
            let value = target_value.ok_or_else(|| {
                "target_value is required when target_identifier is set".to_string()
            })?;
            format!("#WRITE:{};{};{};{}", new_epc, password, identifier, value)
        } else {
            format!("#WRITE:{};{}", new_epc, password)
        };
        self.write(&command).await
    }

    pub async fn write_gpo(
        &self,
        pin: u8,
        state: bool,
        control: &str,
        time_ms: u64,
    ) -> Result<(), String> {
        if !(1..=3).contains(&pin) {
            return Err("Pin must be between 1 and 3".to_string());
        }
        if control != "static" && control != "pulsed" {
            return Err("Control must be 'static' or 'pulsed'".to_string());
        }

        let cmd_on = format!("#GPO:{},{}", pin, if state { "ON" } else { "OFF" });
        if control == "static" {
            return self.write(&cmd_on).await;
        }

        let cmd_off = format!("#GPO:{},{}", pin, if state { "OFF" } else { "ON" });
        self.write(&cmd_on).await?;
        sleep(Duration::from_millis(time_ms)).await;
        self.write(&cmd_off).await
    }
}
