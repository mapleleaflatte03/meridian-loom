use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use ureq::ResponseExt;
use url::Url;
use wasmtime::{
    Caller, Config, Engine, Instance, InstanceAllocationStrategy, Linker, Module,
    PoolingAllocationConfig, Store, StoreLimitsBuilder, Val,
};

use super::{
    host_config_hints, parse_wasm_browser_navigate_request, parse_wasm_heartbeat_schedule_request,
    parse_wasm_terminal_exec_request, render_wasm_browser_navigate_response_json,
    render_wasm_heartbeat_schedule_response_json, render_wasm_terminal_exec_response_json,
    WasmBrowserNavigateResponse, WasmHeartbeatScheduleResponse, WasmHostCallDecision,
    WasmHostCallStatusCode, WasmHostConfig, WasmTerminalExecResponse,
    HOST_BROWSER_NAVIGATE, HOST_SCHEDULE_HEARTBEAT, HOST_TERMINAL_EXEC,
    WASM_HOST_CALL_NAMESPACE, WASM_HOST_RESULT_LEN_EXPORT, WASM_HOST_RESULT_PTR_EXPORT,
};

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
    pub host_response_json: Option<String>,
    pub host_calls: Vec<String>,
    pub notes: Vec<String>,
}

struct RunnerState {
    limits: wasmtime::StoreLimits,
    host_calls: Vec<String>,
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
            host_calls: Vec::new(),
        },
    );
    store.limiter(|state| &mut state.limits);
    if request.host.fuel_metering_enabled {
        store
            .add_fuel(request.fuel_budget)
            .map_err(|error| format!("failed to set fuel: {error}"))?;
    }

    let mut linker = Linker::new(&engine);
    linker
        .func_wrap(
            WASM_HOST_CALL_NAMESPACE,
            HOST_BROWSER_NAVIGATE,
            |caller: Caller<'_, RunnerState>, request_ptr: i32, request_len: i32, response_ptr: i32, response_capacity: i32| {
                host_browser_navigate(caller, request_ptr, request_len, response_ptr, response_capacity)
            },
        )
        .map_err(|error| format!("failed to define {HOST_BROWSER_NAVIGATE}: {error}"))?;
    linker
        .func_wrap(
            WASM_HOST_CALL_NAMESPACE,
            HOST_SCHEDULE_HEARTBEAT,
            |caller: Caller<'_, RunnerState>, request_ptr: i32, request_len: i32, response_ptr: i32, response_capacity: i32| {
                host_schedule_heartbeat(caller, request_ptr, request_len, response_ptr, response_capacity)
            },
        )
        .map_err(|error| format!("failed to define {HOST_SCHEDULE_HEARTBEAT}: {error}"))?;
    linker
        .func_wrap(
            WASM_HOST_CALL_NAMESPACE,
            HOST_TERMINAL_EXEC,
            |caller: Caller<'_, RunnerState>, request_ptr: i32, request_len: i32, response_ptr: i32, response_capacity: i32| {
                host_terminal_exec(caller, request_ptr, request_len, response_ptr, response_capacity)
            },
        )
        .map_err(|error| format!("failed to define {HOST_TERMINAL_EXEC}: {error}"))?;

    let instance = linker
        .instantiate(&mut store, &module)
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

    let host_response_json = capture_host_response_json(&mut store, &instance)?;
    let host_calls = store.data().host_calls.clone();

    let mut notes = vec![
        "experimental local Wasmtime guest execution".to_string(),
        "truth boundary: local-only execution path, not hosted runtime replacement".to_string(),
    ];
    if !host_calls.is_empty() {
        notes.push(format!("host calls dispatched: {}", host_calls.join(", ")));
    }
    if host_response_json.is_some() {
        notes.push("host-call response captured from guest memory".to_string());
    }

    Ok(WasmExecutionResult {
        host_backend: request.host.backend.label().to_string(),
        host_profile_name: request.host.profile_name.clone(),
        runtime_path: if host_calls.is_empty() {
            "wasmtime_local_guest".to_string()
        } else {
            "wasmtime_local_guest_with_host_calls".to_string()
        },
        module_name,
        entrypoint: request.entrypoint.clone(),
        entrypoint_result,
        memory_probe_export,
        memory_probe_result,
        memory_pages_after,
        store_memory_limit_bytes: request.host.store_limits.max_memory_bytes,
        pooling_profile: request.host.pooling.profile.label().to_string(),
        host_hints: host_config_hints(&request.host),
        host_response_json,
        host_calls,
        notes,
    })
}

fn host_browser_navigate(
    mut caller: Caller<'_, RunnerState>,
    request_ptr: i32,
    request_len: i32,
    response_ptr: i32,
    response_capacity: i32,
) -> i32 {
    caller.data_mut().host_calls.push("browser.navigate".to_string());
    match dispatch_browser_navigate(&mut caller, request_ptr, request_len, response_ptr, response_capacity) {
        Ok(bytes_written) => bytes_written,
        Err(status) => status.code(),
    }
}

fn host_schedule_heartbeat(
    mut caller: Caller<'_, RunnerState>,
    request_ptr: i32,
    request_len: i32,
    response_ptr: i32,
    response_capacity: i32,
) -> i32 {
    caller.data_mut().host_calls.push("heartbeat.schedule".to_string());
    match dispatch_schedule_heartbeat(&mut caller, request_ptr, request_len, response_ptr, response_capacity) {
        Ok(bytes_written) => bytes_written,
        Err(status) => status.code(),
    }
}

fn host_terminal_exec(
    mut caller: Caller<'_, RunnerState>,
    request_ptr: i32,
    request_len: i32,
    response_ptr: i32,
    response_capacity: i32,
) -> i32 {
    caller.data_mut().host_calls.push("terminal.exec".to_string());
    match dispatch_terminal_exec(&mut caller, request_ptr, request_len, response_ptr, response_capacity) {
        Ok(bytes_written) => bytes_written,
        Err(status) => status.code(),
    }
}

fn dispatch_browser_navigate(
    caller: &mut Caller<'_, RunnerState>,
    request_ptr: i32,
    request_len: i32,
    response_ptr: i32,
    response_capacity: i32,
) -> Result<i32, WasmHostCallStatusCode> {
    let memory = guest_memory(caller)?;
    let request_json = read_guest_utf8(&memory, caller, request_ptr, request_len)?;
    let request = parse_wasm_browser_navigate_request(&request_json)
        .map_err(|_| WasmHostCallStatusCode::InvalidRequest)?;

    let url = Url::parse(&request.url).map_err(|_| WasmHostCallStatusCode::InvalidRequest)?;
    if !matches!(url.scheme(), "http" | "https") {
        let response = WasmBrowserNavigateResponse {
            decision: WasmHostCallDecision::Denied,
            navigation_id: request.security.operation_id.clone(),
            final_url: request.url.clone(),
            http_status: None,
            content_type: String::new(),
            title: String::new(),
            body_excerpt_utf8: String::new(),
            semantic_snapshot_ref: String::new(),
            note: format!("unsupported scheme '{}'", url.scheme()),
        };
        return write_guest_json_response(&memory, caller, response_ptr, response_capacity, &render_wasm_browser_navigate_response_json(&response));
    }

    let host = url.host_str().unwrap_or_default().to_string();
    let allowed_hosts = if request.allowed_hosts.is_empty() {
        request.security.allowed_hosts.clone()
    } else {
        request.allowed_hosts.clone()
    };
    if !allowed_hosts.is_empty() && !allowed_hosts.iter().any(|value| value == &host) {
        let response = WasmBrowserNavigateResponse {
            decision: WasmHostCallDecision::Denied,
            navigation_id: request.security.operation_id.clone(),
            final_url: request.url.clone(),
            http_status: None,
            content_type: String::new(),
            title: String::new(),
            body_excerpt_utf8: String::new(),
            semantic_snapshot_ref: String::new(),
            note: format!("host '{}' is outside the allowed host set", host),
        };
        return write_guest_json_response(&memory, caller, response_ptr, response_capacity, &render_wasm_browser_navigate_response_json(&response));
    }

    let timeout_ms = bounded_timeout_ms(request.timeout_ms, request.security.max_timeout_ms);
    let max_response_bytes = bounded_response_bytes(request.security.max_response_bytes, response_capacity);
    let mut response = match ureq::get(request.url.as_str())
        .config()
        .timeout_global(Some(Duration::from_millis(timeout_ms)))
        .http_status_as_error(false)
        .build()
        .call()
    {
        Ok(response) => response,
        Err(error) => {
            let response = WasmBrowserNavigateResponse {
                decision: WasmHostCallDecision::Denied,
                navigation_id: request.security.operation_id.clone(),
                final_url: request.url.clone(),
                http_status: None,
                content_type: String::new(),
                title: String::new(),
                body_excerpt_utf8: String::new(),
                semantic_snapshot_ref: String::new(),
                note: format!("http navigate failed: {error}"),
            };
            return write_guest_json_response(&memory, caller, response_ptr, response_capacity, &render_wasm_browser_navigate_response_json(&response));
        }
    };

    let final_url = response.get_uri().to_string();
    let http_status = Some(response.status().as_u16());
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();
    let body_excerpt_utf8 = response
        .body_mut()
        .with_config()
        .limit(max_response_bytes as u64)
        .lossy_utf8(true)
        .read_to_string()
        .unwrap_or_else(|error| format!("[body read failed: {error}]"));
    let title = extract_html_title(&body_excerpt_utf8);
    let response = WasmBrowserNavigateResponse {
        decision: WasmHostCallDecision::Allowed,
        navigation_id: if request.security.operation_id.trim().is_empty() {
            request.session_id.clone()
        } else {
            request.security.operation_id.clone()
        },
        final_url,
        http_status,
        content_type,
        title,
        body_excerpt_utf8: body_excerpt_utf8.clone(),
        semantic_snapshot_ref: if request.capture_semantic_snapshot {
            format!("inline:body_excerpt:{}", body_excerpt_utf8.len())
        } else {
            String::new()
        },
        note: format!("bounded synchronous browser host call completed within {}ms", timeout_ms),
    };
    write_guest_json_response(&memory, caller, response_ptr, response_capacity, &render_wasm_browser_navigate_response_json(&response))
}

fn dispatch_schedule_heartbeat(
    caller: &mut Caller<'_, RunnerState>,
    request_ptr: i32,
    request_len: i32,
    response_ptr: i32,
    response_capacity: i32,
) -> Result<i32, WasmHostCallStatusCode> {
    let memory = guest_memory(caller)?;
    let request_json = read_guest_utf8(&memory, caller, request_ptr, request_len)?;
    let request = parse_wasm_heartbeat_schedule_request(&request_json)
        .map_err(|_| WasmHostCallStatusCode::InvalidRequest)?;
    let response = WasmHeartbeatScheduleResponse {
        decision: WasmHostCallDecision::Denied,
        heartbeat_id: request.heartbeat_id,
        next_fire_at_unix_ms: None,
        accepted_run_id: String::new(),
        note: "heartbeat scheduling remains a declared surface until a runtime service owns leases and acknowledgements".to_string(),
    };
    write_guest_json_response(&memory, caller, response_ptr, response_capacity, &render_wasm_heartbeat_schedule_response_json(&response))
}

fn dispatch_terminal_exec(
    caller: &mut Caller<'_, RunnerState>,
    request_ptr: i32,
    request_len: i32,
    response_ptr: i32,
    response_capacity: i32,
) -> Result<i32, WasmHostCallStatusCode> {
    let memory = guest_memory(caller)?;
    let request_json = read_guest_utf8(&memory, caller, request_ptr, request_len)?;
    let request = parse_wasm_terminal_exec_request(&request_json)
        .map_err(|_| WasmHostCallStatusCode::InvalidRequest)?;

    if request.argv.is_empty() {
        let response = WasmTerminalExecResponse {
            decision: WasmHostCallDecision::Denied,
            exit_code: None,
            stdout_utf8: String::new(),
            stderr_utf8: String::new(),
            timed_out: false,
            truncated: false,
            note: "argv must contain at least one executable".to_string(),
        };
        return write_guest_json_response(&memory, caller, response_ptr, response_capacity, &render_wasm_terminal_exec_response_json(&response));
    }

    let workdir = match resolve_allowed_workdir(&request.working_dir, &request.security.allowed_workdir_roots) {
        Ok(path) => path,
        Err(error) => {
            let response = WasmTerminalExecResponse {
                decision: WasmHostCallDecision::Denied,
                exit_code: None,
                stdout_utf8: String::new(),
                stderr_utf8: String::new(),
                timed_out: false,
                truncated: false,
                note: error,
            };
            return write_guest_json_response(&memory, caller, response_ptr, response_capacity, &render_wasm_terminal_exec_response_json(&response));
        }
    };

    let timeout_ms = bounded_timeout_ms(request.timeout_ms, request.security.max_timeout_ms);
    let max_output_bytes = bounded_response_bytes(request.max_output_bytes.min(request.security.max_response_bytes), response_capacity);
    let mut command = Command::new(&request.argv[0]);
    command.args(&request.argv[1..]);
    command.current_dir(&workdir);
    command.stdin(Stdio::piped());
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());
    if request.require_clean_environment {
        command.env_clear();
    }
    for name in &request.env_allowlist {
        if let Ok(value) = std::env::var(name) {
            command.env(name, value);
        }
    }

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            let response = WasmTerminalExecResponse {
                decision: WasmHostCallDecision::Denied,
                exit_code: None,
                stdout_utf8: String::new(),
                stderr_utf8: String::new(),
                timed_out: false,
                truncated: false,
                note: format!("failed to spawn '{}': {error}", request.argv[0]),
            };
            return write_guest_json_response(&memory, caller, response_ptr, response_capacity, &render_wasm_terminal_exec_response_json(&response));
        }
    };

    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(request.stdin_utf8.as_bytes());
    }

    let started_at = Instant::now();
    let timed_out = loop {
        match child.try_wait() {
            Ok(Some(_)) => break false,
            Ok(None) => {
                if started_at.elapsed() >= Duration::from_millis(timeout_ms) {
                    let _ = child.kill();
                    break true;
                }
                thread::sleep(Duration::from_millis(10));
            }
            Err(_) => break false,
        }
    };

    let output = child
        .wait_with_output()
        .map_err(|_| WasmHostCallStatusCode::InternalError)?;
    let (stdout_utf8, stdout_truncated) = truncate_lossy_utf8(&output.stdout, max_output_bytes);
    let (stderr_utf8, stderr_truncated) = truncate_lossy_utf8(&output.stderr, max_output_bytes);
    let response = WasmTerminalExecResponse {
        decision: WasmHostCallDecision::Allowed,
        exit_code: output.status.code(),
        stdout_utf8,
        stderr_utf8,
        timed_out,
        truncated: stdout_truncated || stderr_truncated,
        note: format!("bounded terminal host call completed in {:?} inside {}", started_at.elapsed(), workdir.display()),
    };
    write_guest_json_response(&memory, caller, response_ptr, response_capacity, &render_wasm_terminal_exec_response_json(&response))
}

fn capture_host_response_json(
    store: &mut Store<RunnerState>,
    instance: &Instance,
) -> Result<Option<String>, String> {
    let result_len = read_exported_i32(store, instance, WASM_HOST_RESULT_LEN_EXPORT);
    let result_ptr = read_exported_i32(store, instance, WASM_HOST_RESULT_PTR_EXPORT);
    let (Some(result_len), Some(result_ptr)) = (result_len, result_ptr) else {
        return Ok(None);
    };
    if result_len <= 0 || result_ptr < 0 {
        return Ok(None);
    }
    let memory = match instance.get_export(&mut *store, "memory").and_then(|export| export.into_memory()) {
        Some(memory) => memory,
        None => return Ok(None),
    };
    let mut bytes = vec![0_u8; result_len as usize];
    memory
        .read(&*store, result_ptr as usize, &mut bytes)
        .map_err(|error| format!("failed to read guest host response memory: {error}"))?;
    Ok(String::from_utf8(bytes).ok())
}

fn read_exported_i32(store: &mut Store<RunnerState>, instance: &Instance, export_name: &str) -> Option<i32> {
    let global = instance.get_export(&mut *store, export_name)?.into_global()?;
    match global.get(&mut *store) {
        Val::I32(value) => Some(value),
        _ => None,
    }
}

fn guest_memory(caller: &mut Caller<'_, RunnerState>) -> Result<wasmtime::Memory, WasmHostCallStatusCode> {
    caller
        .get_export("memory")
        .and_then(|export| export.into_memory())
        .ok_or(WasmHostCallStatusCode::InternalError)
}

fn read_guest_utf8(
    memory: &wasmtime::Memory,
    caller: &Caller<'_, RunnerState>,
    ptr: i32,
    len: i32,
) -> Result<String, WasmHostCallStatusCode> {
    if ptr < 0 || len < 0 {
        return Err(WasmHostCallStatusCode::InvalidRequest);
    }
    let mut bytes = vec![0_u8; len as usize];
    memory
        .read(caller, ptr as usize, &mut bytes)
        .map_err(|_| WasmHostCallStatusCode::InvalidRequest)?;
    String::from_utf8(bytes).map_err(|_| WasmHostCallStatusCode::InvalidRequest)
}

fn write_guest_json_response(
    memory: &wasmtime::Memory,
    caller: &mut Caller<'_, RunnerState>,
    response_ptr: i32,
    response_capacity: i32,
    response_json: &str,
) -> Result<i32, WasmHostCallStatusCode> {
    if response_ptr < 0 || response_capacity < 0 {
        return Err(WasmHostCallStatusCode::InvalidRequest);
    }
    let bytes = response_json.as_bytes();
    if bytes.len() > response_capacity as usize {
        return Err(WasmHostCallStatusCode::ResponseTooLarge);
    }
    memory
        .write(caller, response_ptr as usize, bytes)
        .map_err(|_| WasmHostCallStatusCode::InternalError)?;
    Ok(bytes.len() as i32)
}

fn bounded_timeout_ms(request_timeout_ms: u64, security_timeout_ms: u64) -> u64 {
    let bounded = request_timeout_ms.min(security_timeout_ms.max(1));
    bounded.max(1)
}

fn bounded_response_bytes(request_bytes: usize, response_capacity: i32) -> usize {
    let capacity = response_capacity.max(256) as usize;
    request_bytes.min(capacity).max(256)
}

fn extract_html_title(body: &str) -> String {
    let lower = body.to_lowercase();
    let Some(start_idx) = lower.find("<title>") else {
        return String::new();
    };
    let remainder = &body[start_idx + 7..];
    let lower_remainder = &lower[start_idx + 7..];
    let Some(end_idx) = lower_remainder.find("</title>") else {
        return String::new();
    };
    remainder[..end_idx].trim().to_string()
}

fn resolve_allowed_workdir(working_dir: &str, allowed_roots: &[String]) -> Result<PathBuf, String> {
    let current_dir = std::env::current_dir().map_err(|error| format!("failed to resolve current dir: {error}"))?;
    let requested = if Path::new(working_dir).is_absolute() {
        PathBuf::from(working_dir)
    } else {
        current_dir.join(working_dir)
    };
    let requested = requested
        .canonicalize()
        .map_err(|error| format!("working_dir '{}' is not accessible: {error}", working_dir))?;
    let roots = if allowed_roots.is_empty() {
        vec![".".to_string()]
    } else {
        allowed_roots.to_vec()
    };
    for root in roots {
        let root_path = if Path::new(&root).is_absolute() {
            PathBuf::from(&root)
        } else {
            current_dir.join(&root)
        };
        if let Ok(canonical_root) = root_path.canonicalize() {
            if requested.starts_with(&canonical_root) {
                return Ok(requested);
            }
        }
    }
    Err(format!(
        "working_dir '{}' is outside the allowed roots {:?}",
        requested.display(),
        allowed_roots
    ))
}

fn truncate_lossy_utf8(bytes: &[u8], max_bytes: usize) -> (String, bool) {
    let limit = max_bytes.max(1);
    let truncated = bytes.len() > limit;
    let slice = if truncated { &bytes[..limit] } else { bytes };
    (String::from_utf8_lossy(slice).into_owned(), truncated)
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
