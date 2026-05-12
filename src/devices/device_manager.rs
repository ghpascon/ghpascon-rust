//! `DeviceManager` — carrega e gerencia múltiplos dispositivos RFID.
//!
//! Cada dispositivo é configurado por um arquivo `.json` em um diretório.
//! O campo `"reader"` determina o tipo (`"X714"` ou `"R700_IOT"`).
//! Todos os outros campos são opcionais — valores padrão são aplicados
//! automaticamente pelo `from_map` de cada device.
//!
//! # Formato mínimo de configuração
//!
//! ```json
//! { "reader": "X714", "connection_type": "TCP", "ip": "192.168.1.50" }
//! ```
//! ```json
//! { "reader": "R700_IOT", "ip": "192.168.1.101" }
//! ```

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde_json::Value;
use tokio::task::JoinHandle;

use super::rfid::r700::R700;
use super::rfid::x714::X714;

// ─── Tipos públicos ───────────────────────────────────────────────────────────

/// Função de callback de eventos: `(device_name, event_type, event_data)`
pub type EventHandler = dyn FnMut(&str, &str, Option<Value>) + Send + 'static;
/// Handler compartilhado entre threads / devices.
pub type SharedEventHandler = Arc<Mutex<Box<EventHandler>>>;

// ─── Enum Device ─────────────────────────────────────────────────────────────

/// Wrapper de enum em torno de todos os tipos de device suportados.
/// `Clone` é barato — todos os tipos usam `Arc` internamente.
pub enum Device {
    X714(X714),
    R700(R700),
}

impl Clone for Device {
    fn clone(&self) -> Self {
        match self {
            Self::X714(d) => Self::X714(d.clone()),
            Self::R700(d) => Self::R700(d.clone()),
        }
    }
}

impl Device {
    pub fn name(&self) -> &str {
        match self {
            Self::X714(d) => &d.config.name,
            Self::R700(d) => &d.config.name,
        }
    }

    pub fn device_type(&self) -> &'static str {
        match self {
            Self::X714(_) => "X714",
            Self::R700(_) => "R700_IOT",
        }
    }

    pub fn is_connected(&self) -> bool {
        match self {
            Self::X714(d) => d.is_connected(),
            Self::R700(d) => d.is_connected(),
        }
    }

    pub fn is_reading(&self) -> bool {
        match self {
            Self::X714(d) => d.is_reading(),
            Self::R700(d) => d.is_reading(),
        }
    }

    pub fn serial_number(&self) -> Option<String> {
        match self {
            Self::X714(d) => d.serial_number(),
            Self::R700(d) => d.serial_number(),
        }
    }

    pub fn connect_instruction(&self) -> String {
        match self {
            Self::X714(d) => d.connect_instruction(),
            Self::R700(d) => d.connect_instruction(),
        }
    }

    pub fn set_event_handler(&mut self, handler: SharedEventHandler) {
        match self {
            Self::X714(d) => d.set_event_handler(handler),
            Self::R700(d) => d.set_event_handler(handler),
        }
    }

    pub async fn connect(&self) {
        match self {
            Self::X714(d) => d.connect().await,
            Self::R700(d) => d.connect().await,
        }
    }

    pub async fn close(&self) {
        match self {
            Self::X714(d) => d.close().await,
            Self::R700(d) => d.close().await,
        }
    }

    pub async fn start_inventory(&self) -> Result<(), String> {
        match self {
            Self::X714(d) => d.start_inventory().await,
            Self::R700(d) => d.start_inventory().await,
        }
    }

    pub async fn stop_inventory(&self) -> Result<(), String> {
        match self {
            Self::X714(d) => d.stop_inventory().await,
            Self::R700(d) => d.stop_inventory().await,
        }
    }

    pub async fn write_epc(
        &self,
        target_identifier: Option<&str>,
        target_value: Option<&str>,
        new_epc: &str,
        password: &str,
    ) -> Result<(), String> {
        match self {
            Self::X714(d) => {
                d.write_epc(target_identifier, target_value, new_epc, password)
                    .await
            }
            Self::R700(d) => {
                d.write_epc(target_identifier, target_value, new_epc, password)
                    .await
            }
        }
    }

    pub async fn write_gpo(
        &self,
        pin: u8,
        state: bool,
        control: &str,
        time_ms: u64,
    ) -> Result<(), String> {
        match self {
            Self::X714(d) => d.write_gpo(pin, state, control, time_ms).await,
            Self::R700(d) => d.write_gpo(pin, state, control, time_ms as u32).await,
        }
    }
}

// ─── DeviceInfo ───────────────────────────────────────────────────────────────

/// Snapshot de estado de um device (não mantém referência ao device).
#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub name: String,
    pub device_type: String,
    pub is_connected: bool,
    pub is_reading: bool,
    pub serial_number: Option<String>,
    pub connect_instruction: String,
}

// ─── DeviceManager ────────────────────────────────────────────────────────────

/// Gerencia múltiplos devices: carrega configs JSON, conecta, despacha eventos.
///
/// # Uso básico
///
/// ```rust,no_run
/// # use ghpascon_rust::devices::device_manager::{DeviceManager, SharedEventHandler};
/// # use std::sync::{Arc, Mutex};
/// # #[tokio::main] async fn main() {
/// let mut manager = DeviceManager::new("examples/devices/configs");
/// manager.load_devices();
/// manager.connect_devices(false).await;
/// # }
/// ```
pub struct DeviceManager {
    /// Devices carregados.
    pub devices: Vec<Device>,
    /// Diretório com os arquivos `.json` de configuração.
    devices_path: PathBuf,
    /// Handler de eventos compartilhado atribuído a todos os devices.
    event_handler: Option<SharedEventHandler>,
    /// Handles das tasks de conexão em background.
    connect_tasks: Vec<JoinHandle<()>>,
}

impl DeviceManager {
    // ─── Construtores ─────────────────────────────────────────────────────────

    /// Cria um manager apontando para `devices_path`.
    pub fn new<P: AsRef<Path>>(devices_path: P) -> Self {
        Self {
            devices: Vec::new(),
            devices_path: devices_path.as_ref().to_path_buf(),
            event_handler: None,
            connect_tasks: Vec::new(),
        }
    }

    /// Define o handler de eventos (builder-style).
    pub fn with_event_handler(mut self, handler: SharedEventHandler) -> Self {
        self.event_handler = Some(handler);
        self
    }

    /// Substitui o handler de eventos.
    pub fn set_event_handler(&mut self, handler: SharedEventHandler) {
        self.event_handler = Some(handler);
    }

    // ─── Carregamento ─────────────────────────────────────────────────────────

    /// Lê os arquivos `.json` de `devices_path` e popula `self.devices`.
    ///
    /// Cada arquivo deve ter um campo `"reader"` (`"X714"` ou `"R700_IOT"`).
    /// O nome do arquivo (sem `.json`) vira o `name` do device.
    /// Todos os outros campos são opcionais — padrões são aplicados automaticamente.
    /// Chama `assign_event_handler()` ao final.
    pub fn load_devices(&mut self) {
        self.devices.clear();

        // Cria o diretório se não existir
        if !self.devices_path.exists() {
            match std::fs::create_dir_all(&self.devices_path) {
                Ok(_) => eprintln!("📁 Diretório criado: {}", self.devices_path.display()),
                Err(e) => {
                    eprintln!(
                        "❌ Não foi possível criar diretório '{}': {e}",
                        self.devices_path.display()
                    );
                    return;
                }
            }
        }

        let entries = match std::fs::read_dir(&self.devices_path) {
            Ok(e) => e,
            Err(e) => {
                eprintln!("❌ Erro ao listar '{}': {e}", self.devices_path.display());
                return;
            }
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }

            let filename = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let name = filename.trim_end_matches(".json").to_string();

            eprintln!("📄 Lendo '{}'…", filename);

            let content = match std::fs::read_to_string(&path) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("❌ Erro ao ler '{}': {e}", filename);
                    continue;
                }
            };

            let raw: HashMap<String, Value> = match serde_json::from_str(&content) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("❌ JSON inválido em '{}': {e}", filename);
                    continue;
                }
            };

            // Normaliza chaves para lowercase
            let data: HashMap<String, Value> = raw
                .into_iter()
                .map(|(k, v)| (k.to_lowercase(), v))
                .collect();

            let reader_type = match data.get("reader").and_then(|v| v.as_str()) {
                Some(t) => t.to_string(),
                None => {
                    eprintln!("⚠️  '{}' não tem campo 'reader' — ignorado", filename);
                    continue;
                }
            };

            self.add_device(&name, &reader_type, data);
        }

        self.assign_event_handler();
        eprintln!("✅ {} device(s) carregado(s)", self.devices.len());
    }

    /// Adiciona um device a partir de parâmetros já parseados.
    ///
    /// O campo `"name"` é injetado automaticamente a partir do nome do arquivo.
    /// Campos não informados usam os padrões de cada device.
    pub fn add_device(&mut self, name: &str, device_type: &str, mut data: HashMap<String, Value>) {
        // Nome vem sempre do arquivo; sobrescreve qualquer valor no JSON
        data.insert("name".to_string(), Value::String(name.to_string()));

        match device_type.to_uppercase().as_str() {
            "X714" => match X714::from_map(data) {
                Ok(d) => {
                    eprintln!("  ✅ X714 '{}' → {}", name, d.connect_instruction());
                    self.devices.push(Device::X714(d));
                }
                Err(e) => eprintln!("  ❌ X714 '{}' erro de config: {e}", name),
            },
            "R700_IOT" | "R700" => match R700::from_map(data) {
                Ok(d) => {
                    eprintln!("  ✅ R700 '{}' → {}", name, d.connect_instruction());
                    self.devices.push(Device::R700(d));
                }
                Err(e) => eprintln!("  ❌ R700 '{}' erro de config: {e}", name),
            },
            other => {
                eprintln!(
                    "  ⚠️  Tipo desconhecido '{}' para '{}' — ignorado",
                    other, name
                );
            }
        }
    }

    // ─── Event handler ────────────────────────────────────────────────────────

    /// Atribui o handler de eventos a todos os devices carregados.
    pub fn assign_event_handler(&mut self) {
        let Some(handler) = &self.event_handler else {
            return;
        };
        for device in &mut self.devices {
            device.set_event_handler(Arc::clone(handler));
        }
    }

    // ─── Conexão ─────────────────────────────────────────────────────────────

    /// Conecta todos os devices em background (tasks tokio).
    ///
    /// - Se já existirem tasks ativas e `force` for `false`, é um no-op.
    /// - Se `force` for `true`, cancela as tasks anteriores, desconecta os
    ///   devices, recarrega os JSONs e reconecta tudo.
    pub async fn connect_devices(&mut self, force: bool) {
        let active = self
            .connect_tasks
            .iter()
            .filter(|t| !t.is_finished())
            .count();

        if active > 0 && !force {
            eprintln!(
                "ℹ️  {} task(s) de conexão ativas — use force=true para reiniciar",
                active
            );
            return;
        }

        self.cancel_connect_tasks().await;
        self.disconnect_devices().await;
        self.load_devices();

        let mut tasks = Vec::new();
        for device in &self.devices {
            let d = device.clone();
            let name_display = d.connect_instruction();
            eprintln!("🚀 Conectando '{}'…  ({})", d.name(), name_display);
            let handle = tokio::spawn(async move { d.connect().await });
            tasks.push(handle);
        }

        eprintln!("ℹ️  {} task(s) de conexão iniciadas", tasks.len());
        self.connect_tasks = tasks;
    }

    /// Cancela todas as tasks de conexão ativas.
    pub async fn cancel_connect_tasks(&mut self) {
        let n = self.connect_tasks.len();
        for task in self.connect_tasks.drain(..) {
            task.abort();
        }
        if n > 0 {
            eprintln!("🛑 {} task(s) cancelada(s)", n);
        }
    }

    /// Fecha todos os devices e limpa a lista.
    pub async fn disconnect_devices(&mut self) {
        for device in &self.devices {
            device.close().await;
        }
        self.devices.clear();
    }

    // ─── Introspection ────────────────────────────────────────────────────────

    pub fn len(&self) -> usize {
        self.devices.len()
    }

    pub fn is_empty(&self) -> bool {
        self.devices.is_empty()
    }

    /// Retorna os nomes de todos os devices carregados.
    pub fn get_device_names(&self) -> Vec<String> {
        self.devices.iter().map(|d| d.name().to_string()).collect()
    }

    /// Retorna referência para um device pelo nome.
    pub fn get_device(&self, name: &str) -> Option<&Device> {
        self.devices.iter().find(|d| d.name() == name)
    }

    /// Retorna referência mutável para um device pelo nome.
    pub fn get_device_mut(&mut self, name: &str) -> Option<&mut Device> {
        self.devices.iter_mut().find(|d| d.name() == name)
    }

    /// Snapshot de estado de um ou todos os devices.
    ///
    /// Se `name` for `None`, retorna info de todos.
    pub fn get_device_info(&self, name: Option<&str>) -> Vec<DeviceInfo> {
        match name {
            Some(n) => self
                .get_device(n)
                .map(|d| vec![Self::build_info(d)])
                .unwrap_or_default(),
            None => self.devices.iter().map(Self::build_info).collect(),
        }
    }

    fn build_info(d: &Device) -> DeviceInfo {
        DeviceInfo {
            name: d.name().to_string(),
            device_type: d.device_type().to_string(),
            is_connected: d.is_connected(),
            is_reading: d.is_reading(),
            serial_number: d.serial_number(),
            connect_instruction: d.connect_instruction(),
        }
    }

    /// `true` se pelo menos um device está conectado e lendo.
    pub fn any_device_reading(&self) -> bool {
        self.devices
            .iter()
            .any(|d| d.is_connected() && d.is_reading())
    }

    /// Retorna o número de serial do device (apenas se conectado).
    pub fn get_serial_number(&self, name: &str) -> Option<String> {
        let d = self.get_device(name)?;
        if !d.is_connected() {
            return None;
        }
        d.serial_number()
    }

    // ─── Inventário ───────────────────────────────────────────────────────────

    /// Inicia inventário em um device específico.
    pub async fn start_inventory(&self, name: &str) -> Result<(), String> {
        let d = self
            .get_device(name)
            .ok_or_else(|| format!("device '{}' não encontrado", name))?;
        if !d.is_connected() {
            return Err(format!("device '{}' não está conectado", name));
        }
        d.start_inventory().await.map_err(|e| {
            eprintln!("❌ start_inventory '{}': {e}", name);
            e
        })
    }

    /// Para inventário em um device específico.
    pub async fn stop_inventory(&self, name: &str) -> Result<(), String> {
        let d = self
            .get_device(name)
            .ok_or_else(|| format!("device '{}' não encontrado", name))?;
        if !d.is_connected() {
            return Err(format!("device '{}' não está conectado", name));
        }
        d.stop_inventory().await.map_err(|e| {
            eprintln!("❌ stop_inventory '{}': {e}", name);
            e
        })
    }

    /// Inicia inventário em todos os devices RFID conectados.
    /// Retorna mapa `name → ok`.
    pub async fn start_inventory_all(&self) -> HashMap<String, bool> {
        let mut results = HashMap::new();
        for d in &self.devices {
            if d.is_connected() {
                let ok = d.start_inventory().await.is_ok();
                results.insert(d.name().to_string(), ok);
            }
        }
        results
    }

    /// Para inventário em todos os devices RFID conectados.
    /// Retorna mapa `name → ok`.
    pub async fn stop_inventory_all(&self) -> HashMap<String, bool> {
        let mut results = HashMap::new();
        for d in &self.devices {
            if d.is_connected() {
                let ok = d.stop_inventory().await.is_ok();
                results.insert(d.name().to_string(), ok);
            }
        }
        results
    }

    // ─── Write operations ─────────────────────────────────────────────────────

    /// Escreve um novo EPC em uma tag.
    ///
    /// - `target_identifier`: `Some("epc")` ou `Some("tid")` (filtra a tag alvo); `None` = qualquer tag.
    /// - `target_value`: valor do identificador alvo (obrigatório quando `target_identifier` é `Some`).
    pub async fn write_epc(
        &self,
        name: &str,
        target_identifier: Option<&str>,
        target_value: Option<&str>,
        new_epc: &str,
        password: &str,
    ) -> Result<(), String> {
        let d = self
            .get_device(name)
            .ok_or_else(|| format!("device '{}' não encontrado", name))?;
        if !d.is_connected() {
            return Err(format!("device '{}' não está conectado", name));
        }
        d.write_epc(target_identifier, target_value, new_epc, password)
            .await
    }

    /// Controla um pino GPO.
    ///
    /// - `control`: `"static"` ou `"pulsed"`
    /// - `time_ms`: duração do pulso (ignorado em modo `"static"`)
    pub async fn write_gpo(
        &self,
        name: &str,
        pin: u8,
        state: bool,
        control: &str,
        time_ms: u64,
    ) -> Result<(), String> {
        let d = self
            .get_device(name)
            .ok_or_else(|| format!("device '{}' não encontrado", name))?;
        if !d.is_connected() {
            return Err(format!("device '{}' não está conectado", name));
        }
        d.write_gpo(pin, state, control, time_ms).await
    }
}
