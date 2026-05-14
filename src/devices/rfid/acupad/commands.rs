use tokio::time::{Duration, sleep};

use super::acupad::Acupad;

impl Acupad {
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
            if self.config.start_reading { "on" } else { "off" }
        ));
        cmds.push(format!(
            "#gpi_start:{}",
            if self.config.gpi_start { "on" } else { "off" }
        ));
        cmds.push(format!(
            "#beep:{}",
            if self.config.beep { "on" } else { "off" }
        ));
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
        for cmd in self.config_commands() {
            self.write(&cmd).await?;
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

    pub async fn protected_inventory(&self, active: bool, password: Option<&str>) -> Result<(), String> {
        let cmd = self.protected_inventory_command(active, password);
        self.write(&cmd).await
    }
}
