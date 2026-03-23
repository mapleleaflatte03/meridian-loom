//! Host-side Wasm configuration plumbing.
//!
//! This module is intentionally honest: it prepares host configuration shapes
//! that can later be wired into Wasmtime or another engine, but it does not
//! pretend to execute components today.

use std::collections::BTreeMap;

use crate::wasm_limits::{default_limits, render_limits_human, validate_limits, WasmStoreLimits};
use crate::wasm_profiles::{render_pooling_config_human, PoolingConfig, PoolingProfile};

#[path = "wasm_runner.rs"]
mod wasm_runner;

#[allow(unused_imports)]
pub use wasm_runner::{
    run_wasm_guest, WasmExecutionRequest, WasmExecutionResult, WasmGuestSource,
    WasmMemoryProbe,
};

/// Host-side runtime backend selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostBackend {
    /// Current truth: local preview / preparation path only.
    PreviewOnly,
    /// Prepared for a Wasmtime-backed runtime wiring.
    WasmtimeReady,
}

impl HostBackend {
    pub fn label(self) -> &'static str {
        match self {
            Self::PreviewOnly => "preview_only",
            Self::WasmtimeReady => "wasmtime_ready",
        }
    }
}

/// A host-side Wasm execution plan with store limits, pooling profile, and
/// engine-preparation metadata.
#[derive(Clone, Debug)]
pub struct WasmHostConfig {
    pub profile_name: String,
    pub backend: HostBackend,
    pub component_model_enabled: bool,
    pub store_limits: WasmStoreLimits,
    pub pooling: PoolingConfig,
    pub fuel_metering_enabled: bool,
    pub epoch_deadline_ms: Option<u64>,
    pub notes: Vec<String>,
}

impl WasmHostConfig {
    pub fn host_memory_budget_bytes(&self) -> u64 {
        self.store_limits.max_memory_bytes
    }

    pub fn per_instance_memory_budget_bytes(&self) -> u64 {
        self.pooling.max_memory_pages as u64 * 65_536
    }

    pub fn pool_memory_budget_bytes(&self) -> u64 {
        self.pooling.total_memory_bytes()
    }
}

/// Builder for a host-side Wasm configuration.
#[derive(Clone, Debug)]
pub struct WasmHostBuilder {
    profile_name: String,
    backend: HostBackend,
    component_model_enabled: bool,
    store_limits: WasmStoreLimits,
    pooling: PoolingConfig,
    fuel_metering_enabled: bool,
    epoch_deadline_ms: Option<u64>,
    notes: Vec<String>,
}

impl Default for WasmHostBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl WasmHostBuilder {
    pub fn new() -> Self {
        Self {
            profile_name: "standard".to_string(),
            backend: HostBackend::PreviewOnly,
            component_model_enabled: true,
            store_limits: default_limits(),
            pooling: PoolingConfig::from_profile(PoolingProfile::Standard),
            fuel_metering_enabled: true,
            epoch_deadline_ms: Some(1_000),
            notes: vec!["prepared host config; execution lives in the experimental guest lane".to_string()],
        }
    }

    pub fn with_profile_name(mut self, profile_name: impl Into<String>) -> Self {
        self.profile_name = profile_name.into();
        self
    }

    pub fn with_backend(mut self, backend: HostBackend) -> Self {
        self.backend = backend;
        self
    }

    pub fn with_component_model_enabled(mut self, enabled: bool) -> Self {
        self.component_model_enabled = enabled;
        self
    }

    pub fn with_store_limits(mut self, limits: WasmStoreLimits) -> Self {
        self.store_limits = limits;
        self
    }

    pub fn with_pooling_profile(mut self, profile: PoolingProfile) -> Self {
        self.pooling = PoolingConfig::from_profile(profile);
        self
    }

    pub fn with_pooling_config(mut self, pooling: PoolingConfig) -> Self {
        self.pooling = pooling;
        self
    }

    pub fn with_fuel_metering_enabled(mut self, enabled: bool) -> Self {
        self.fuel_metering_enabled = enabled;
        self
    }

    pub fn with_epoch_deadline_ms(mut self, deadline: Option<u64>) -> Self {
        self.epoch_deadline_ms = deadline;
        self
    }

    pub fn add_note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }

    pub fn build(self) -> Result<WasmHostConfig, Vec<String>> {
        let mut errors = Vec::new();

        if let Err(mut reasons) = validate_limits(&self.store_limits) {
            errors.append(&mut reasons);
        }
        if let Err(reason) = self.pooling.validate() {
            errors.push(reason);
        }
        let per_instance_budget = self.pooling.max_memory_pages as u64 * 65_536;
        if per_instance_budget > self.store_limits.max_memory_bytes {
            errors.push(format!(
                "pooling profile can allocate {} bytes per instance, which exceeds store limit {} bytes",
                per_instance_budget,
                self.store_limits.max_memory_bytes
            ));
        }
        if self.profile_name.trim().is_empty() {
            errors.push("profile_name must not be empty".to_string());
        }

        if errors.is_empty() {
            Ok(WasmHostConfig {
                profile_name: self.profile_name,
                backend: self.backend,
                component_model_enabled: self.component_model_enabled,
                store_limits: self.store_limits,
                pooling: self.pooling,
                fuel_metering_enabled: self.fuel_metering_enabled,
                epoch_deadline_ms: self.epoch_deadline_ms,
                notes: self.notes,
            })
        } else {
            Err(errors)
        }
    }

    pub fn build_with_profile(profile_name: impl Into<String>, profile: PoolingProfile) -> Result<WasmHostConfig, Vec<String>> {
        Self::new()
            .with_profile_name(profile_name)
            .with_pooling_profile(profile)
            .build()
    }
}

/// Build a concise host configuration map for later Wasmtime wiring.
pub fn host_config_hints(config: &WasmHostConfig) -> BTreeMap<String, String> {
    let mut hints = BTreeMap::new();
    hints.insert("profile_name".to_string(), config.profile_name.clone());
    hints.insert("backend".to_string(), config.backend.label().to_string());
    hints.insert(
        "component_model_enabled".to_string(),
        config.component_model_enabled.to_string(),
    );
    hints.insert(
        "fuel_metering_enabled".to_string(),
        config.fuel_metering_enabled.to_string(),
    );
    hints.insert(
        "epoch_deadline_ms".to_string(),
        config
            .epoch_deadline_ms
            .map(|value| value.to_string())
            .unwrap_or_else(|| "none".to_string()),
    );
    hints.insert(
        "store_memory_budget_bytes".to_string(),
        config.host_memory_budget_bytes().to_string(),
    );
    hints.insert(
        "per_instance_memory_budget_bytes".to_string(),
        config.per_instance_memory_budget_bytes().to_string(),
    );
    hints.insert(
        "pool_memory_budget_bytes".to_string(),
        config.pool_memory_budget_bytes().to_string(),
    );
    hints.insert(
        "pool_profile".to_string(),
        config.pooling.profile.label().to_string(),
    );
    hints
}

pub fn render_host_config_human(config: &WasmHostConfig) -> String {
    let hints = host_config_hints(config);
    let mut text = String::from(
        "Meridian Loom // WASM HOST CONFIG\n\
         =================================\n",
    );
    text.push_str(&format!(
        "profile              {}\nbackend              {}\ncomponent_model      {}\nfuel_metering        {}\nepoch_deadline_ms    {}\n",
        config.profile_name,
        config.backend.label(),
        config.component_model_enabled,
        config.fuel_metering_enabled,
        config.epoch_deadline_ms
            .map(|v| v.to_string())
            .unwrap_or_else(|| "none".to_string()),
    ));
    text.push_str("store_limits\n");
    text.push_str(&render_limits_human(&config.store_limits));
    text.push('\n');
    text.push_str("pooling_profile\n");
    text.push_str(&render_pooling_config_human(&config.pooling));
    text.push_str("\nhost_hints\n");
    for (key, value) in hints {
        text.push_str(&format!("{} = {}\n", key, value));
    }
    if !config.notes.is_empty() {
        text.push_str("notes\n");
        for note in &config.notes {
            text.push_str(&format!("- {}\n", note));
        }
    }
    text
}

pub fn render_host_config_json(config: &WasmHostConfig) -> String {
    let epoch = config
        .epoch_deadline_ms
        .map(|value| value.to_string())
        .unwrap_or_else(|| "null".to_string());
    let notes_json = if config.notes.is_empty() {
        "[]".to_string()
    } else {
        let values = config
            .notes
            .iter()
            .map(|note| format!("{:?}", note))
            .collect::<Vec<_>>()
            .join(",");
        format!("[{}]", values)
    };
    format!(
        "{{\n  \"profile_name\": {:?},\n  \"backend\": {:?},\n  \"component_model_enabled\": {},\n  \"fuel_metering_enabled\": {},\n  \"epoch_deadline_ms\": {},\n  \"store_limits\": {},\n  \"pooling\": {},\n  \"notes\": {}\n}}",
        config.profile_name,
        config.backend.label(),
        config.component_model_enabled,
        config.fuel_metering_enabled,
        epoch,
        crate::wasm_limits::render_limits_json(&config.store_limits),
        crate::wasm_profiles::render_pooling_config_json(&config.pooling),
        notes_json,
    )
}

pub fn validate_host_config(config: &WasmHostConfig) -> Result<(), Vec<String>> {
    WasmHostBuilder {
        profile_name: config.profile_name.clone(),
        backend: config.backend,
        component_model_enabled: config.component_model_enabled,
        store_limits: config.store_limits.clone(),
        pooling: config.pooling.clone(),
        fuel_metering_enabled: config.fuel_metering_enabled,
        epoch_deadline_ms: config.epoch_deadline_ms,
        notes: config.notes.clone(),
    }
    .build()
    .map(|_| ())
}

pub fn build_wasmtime_config(config: &WasmHostConfig) -> Result<wasmtime::Config, Vec<String>> {
    wasm_runner::build_wasmtime_config(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_builder_is_truthful() {
        let config = WasmHostBuilder::new().build().expect("default config");
        assert_eq!(config.profile_name, "standard");
        assert_eq!(config.backend, HostBackend::PreviewOnly);
        assert!(config.component_model_enabled);
        assert!(config.fuel_metering_enabled);
        assert!(config.notes.iter().any(|note| note.contains("experimental guest lane")));
        assert!(validate_host_config(&config).is_ok());
    }

    #[test]
    fn builder_can_be_marked_wasmtime_ready_without_faking_execution() {
        let config = WasmHostBuilder::new()
            .with_backend(HostBackend::WasmtimeReady)
            .with_profile_name("host/wasmtime-ready")
            .build()
            .expect("wasmtime-ready config");
        assert_eq!(config.backend, HostBackend::WasmtimeReady);
        assert_eq!(config.profile_name, "host/wasmtime-ready");
        let hints = host_config_hints(&config);
        assert_eq!(hints.get("backend"), Some(&"wasmtime_ready".to_string()));
    }

    #[test]
    fn builder_rejects_pooling_that_exceeds_store_memory() {
        let limits = WasmStoreLimits {
            max_memory_bytes: 1_048_576, // 1 MiB
            max_table_elements: 1_000,
            max_instances: 4,
            max_tables: 4,
            max_memories: 4,
            fuel_limit: Some(100_000),
        };
        let config = WasmHostBuilder::new()
            .with_store_limits(limits)
            .with_pooling_config(PoolingConfig::from_profile(PoolingProfile::Heavy))
            .build()
            .expect_err("should reject oversized pooling");
        assert!(config.iter().any(|reason| reason.contains("exceeds store limit")));
    }

    #[test]
    fn render_human_includes_store_and_pooling_surfaces() {
        let config = WasmHostBuilder::new().build().expect("config");
        let rendered = render_host_config_human(&config);
        assert!(rendered.contains("WASM HOST CONFIG"));
        assert!(rendered.contains("store_limits"));
        assert!(rendered.contains("pooling_profile"));
    }

    #[test]
    fn json_render_is_structured_and_truthful() {
        let config = WasmHostBuilder::new()
            .with_epoch_deadline_ms(Some(2_500))
            .build()
            .expect("config");
        let rendered = render_host_config_json(&config);
        assert!(rendered.contains("\"profile_name\""));
        assert!(rendered.contains("\"epoch_deadline_ms\": 2500"));
        assert!(rendered.contains("\"notes\""));
    }
}
