#[path = "../src/wasm_limits.rs"]
mod wasm_limits;
#[path = "../src/wasm_profiles.rs"]
mod wasm_profiles;
#[path = "../src/wasm_host.rs"]
mod wasm_host;

use wasm_host::{
    host_config_hints, render_host_config_human, render_host_config_json, validate_host_config,
    HostBackend, WasmHostBuilder,
};
use wasm_profiles::{PoolingConfig, PoolingProfile};
use wasm_limits::WasmStoreLimits;

#[test]
fn host_builder_defaults_to_truthful_preview_mode() {
    let config = WasmHostBuilder::new().build().expect("default host config");
    assert_eq!(config.backend, HostBackend::PreviewOnly);
    assert!(config.component_model_enabled);
    assert!(config.fuel_metering_enabled);
    assert!(validate_host_config(&config).is_ok());
}

#[test]
fn host_builder_can_select_profile_and_backend() {
    let config = WasmHostBuilder::new()
        .with_profile_name("host/standard")
        .with_backend(HostBackend::WasmtimeReady)
        .with_pooling_profile(PoolingProfile::Standard)
        .build()
        .expect("host config");
    let hints = host_config_hints(&config);
    assert_eq!(hints.get("profile_name"), Some(&"host/standard".to_string()));
    assert_eq!(hints.get("backend"), Some(&"wasmtime_ready".to_string()));
    assert_eq!(config.pooling.profile, PoolingProfile::Standard);
}

#[test]
fn host_builder_rejects_incompatible_store_and_pooling() {
    let config = WasmHostBuilder::new()
        .with_store_limits(WasmStoreLimits {
            max_memory_bytes: 512 * 1024,
            max_table_elements: 1_000,
            max_instances: 4,
            max_tables: 4,
            max_memories: 4,
            fuel_limit: Some(10_000),
        })
        .with_pooling_config(PoolingConfig::from_profile(PoolingProfile::Heavy))
        .build();
    let errors = config.expect_err("should fail");
    assert!(errors.iter().any(|error| error.contains("exceeds store limit")));
}

#[test]
fn host_rendering_is_terminal_friendly() {
    let config = WasmHostBuilder::new()
        .with_profile_name("terminal/host")
        .with_epoch_deadline_ms(Some(1_500))
        .build()
        .expect("host config");
    let human = render_host_config_human(&config);
    let json = render_host_config_json(&config);
    assert!(human.contains("Meridian Loom // WASM HOST CONFIG"));
    assert!(human.contains("epoch_deadline_ms"));
    assert!(json.contains("\"backend\": \"preview_only\""));
}
