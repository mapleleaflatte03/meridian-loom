use std::collections::BTreeMap;

use wasmtime::{
    Config, Engine, Instance, InstanceAllocationStrategy, Module, PoolingAllocationConfig, Store,
    StoreLimitsBuilder, Val,
};

use super::{host_config_hints, WasmHostConfig};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WasmGuestSource {
    WasmBytes {
        name: String,
        bytes: Vec<u8>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WasmMemoryProbe {
    pub export_name: String,
    pub pages_to_grow: u32,
}

#[derive(Clone, Debug)]
pub struct WasmExecutionRequest {
    pub host: WasmHostConfig,
    pub source: WasmGuestSource,
    pub entrypoint: String,
    pub entrypoint_args: Vec<i32>,
    pub memory_probe: Option<WasmMemoryProbe>,
    pub fuel_budget: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WasmExecutionResult {
    pub host_backend: String,
    pub host_profile_name: String,
    pub runtime_path: String,
    pub module_name: String,
    pub entrypoint: String,
    pub entrypoint_result: Option<i32>,
    pub memory_probe_export: Option<String>,
    pub memory_probe_result: Option<i32>,
    pub memory_pages_after: Option<u32>,
    pub store_memory_limit_bytes: u64,
    pub pooling_profile: String,
    pub host_hints: BTreeMap<String, String>,
    pub notes: Vec<String>,
}

struct RunnerState {
    limits: wasmtime::StoreLimits,
}

pub fn build_wasmtime_config(host: &WasmHostConfig) -> Result<Config, Vec<String>> {
    let mut config = Config::new();

    config.consume_fuel(host.fuel_metering_enabled);

    let mut pooling = PoolingAllocationConfig::default();
    pooling
        .total_core_instances(host.pooling.max_instances)
        .total_memories(host.pooling.max_instances.saturating_mul(host.pooling.max_memories_per_instance))
        .total_tables(host.pooling.max_instances.saturating_mul(host.pooling.max_tables_per_instance))
        .max_core_instances_per_component(host.pooling.max_instances)
        .max_core_instance_size(host.pooling.max_memory_pages as usize * 65_536)
        .max_memories_per_component(host.pooling.max_memories_per_instance)
        .max_memories_per_module(host.pooling.max_memories_per_instance)
        .max_tables_per_component(host.pooling.max_tables_per_instance)
        .max_tables_per_module(host.pooling.max_tables_per_instance)
        .table_elements(host.pooling.max_table_elements);

    config.allocation_strategy(InstanceAllocationStrategy::Pooling(pooling));

    Ok(config)
}

pub fn run_wasm_guest(request: &WasmExecutionRequest) -> Result<WasmExecutionResult, String> {
    let config = build_wasmtime_config(&request.host).map_err(|errors| errors.join("; "))?;
    let engine = Engine::new(&config).map_err(|error| format!("failed to build engine: {error}"))?;
    let wasm_bytes = match &request.source {
        WasmGuestSource::WasmBytes { bytes, .. } => bytes.clone(),
    };

    let module_name = match &request.source {
        WasmGuestSource::WasmBytes { name, .. } => name.clone(),
    };
    let module = Module::new(&engine, &wasm_bytes)
        .map_err(|error| format!("failed to compile module: {error}"))?;
    let store_limits = StoreLimitsBuilder::new()
        .memory_size(request.host.store_limits.max_memory_bytes as usize)
        .table_elements(request.host.store_limits.max_table_elements)
        .instances(request.host.store_limits.max_instances as usize)
        .tables(request.host.store_limits.max_tables as usize)
        .memories(request.host.store_limits.max_memories as usize)
        .trap_on_grow_failure(false)
        .build();
    let mut store = Store::new(
        &engine,
        RunnerState {
            limits: store_limits,
        },
    );
    store.limiter(|state| &mut state.limits);
    if request.host.fuel_metering_enabled {
        store
            .add_fuel(request.fuel_budget)
            .map_err(|error| format!("failed to set fuel: {error}"))?;
    }

    let instance = Instance::new(&mut store, &module, &[])
        .map_err(|error| format!("failed to instantiate module: {error}"))?;
    let func = instance
        .get_func(&mut store, &request.entrypoint)
        .ok_or_else(|| format!("missing export '{}'", request.entrypoint))?;
    let entrypoint_result = call_i32_function(&mut store, &func, &request.entrypoint_args)?;

    let (memory_probe_export, memory_probe_result, memory_pages_after) = if let Some(probe) = &request.memory_probe {
        let probe_func = instance
            .get_func(&mut store, &probe.export_name)
            .ok_or_else(|| format!("missing memory probe export '{}'", probe.export_name))?;
        let result = call_i32_function(&mut store, &probe_func, &[probe.pages_to_grow as i32])?;
        let pages_after = instance
            .get_export(&mut store, "memory")
            .and_then(|export| export.into_memory())
            .map(|memory| memory.size(&store) as u32);
        (Some(probe.export_name.clone()), result, pages_after)
    } else {
        (None, None, None)
    };

    Ok(WasmExecutionResult {
        host_backend: request.host.backend.label().to_string(),
        host_profile_name: request.host.profile_name.clone(),
        runtime_path: "wasmtime_local_guest".to_string(),
        module_name,
        entrypoint: request.entrypoint.clone(),
        entrypoint_result,
        memory_probe_export,
        memory_probe_result,
        memory_pages_after,
        store_memory_limit_bytes: request.host.store_limits.max_memory_bytes,
        pooling_profile: request.host.pooling.profile.label().to_string(),
        host_hints: host_config_hints(&request.host),
        notes: vec![
            "experimental local Wasmtime guest execution".to_string(),
            "truth boundary: local-only execution path, not hosted runtime replacement".to_string(),
        ],
    })
}

fn call_i32_function(
    store: &mut Store<RunnerState>,
    func: &wasmtime::Func,
    args: &[i32],
) -> Result<Option<i32>, String> {
    if args.len() > 1 {
        return Err("experimental local Wasm lane supports at most one i32 argument for now".to_string());
    }
    let mut results = vec![Val::I32(0); func.ty(&*store).results().len()];
    match args.len() {
        0 => func
            .call(store, &[], &mut results)
            .map_err(|error| format!("wasm call failed: {error}"))?,
        1 => func
            .call(store, &[Val::I32(args[0])], &mut results)
            .map_err(|error| format!("wasm call failed: {error}"))?,
        _ => unreachable!(),
    }
    Ok(match results.first() {
        Some(Val::I32(value)) => Some(*value),
        _ => None,
    })
}
