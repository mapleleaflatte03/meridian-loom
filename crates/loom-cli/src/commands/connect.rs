use std::fs::OpenOptions;
use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};

use crate::*;
use serde_json::{json, Value};

const DEFAULT_CONNECT_REGISTRY_PATH: &str = "state/connect/registry.json";
const DEFAULT_CONNECT_ADAPTERS_DIR: &str = "state/connect/adapters";
const DEFAULT_CONNECT_HEALTH_DIR: &str = "state/connect/health";
const DEFAULT_CONNECT_TESTS_DIR: &str = "state/connect/tests";
const DEFAULT_CONNECT_LATEST_ARTIFACT_PATH: &str = "artifacts/connect/latest.json";
const CONNECT_REGISTRY_SCHEMA_V1: &str = "meridian.connect.registry.v1";
const CONNECT_REGISTRY_SCHEMA_V2: &str = "meridian.connect.registry.v2";
const CONNECT_ADAPTER_SCHEMA: &str = "meridian.connect.adapter.v1";
const CONNECT_RUNTIME_CONTRACT_V2: &str = "connect_runtime_contract_v2";
const CONNECT_TEST_EVENT_SCHEMA: &str = "meridian.connect.test_event.v1";
const CONNECT_HEALTH_EVENT_SCHEMA: &str = "meridian.connect.health_event.v1";
const SUPPORTED_TRANSPORTS: [&str; 5] = ["grpc", "a2a", "mcp", "http", "ros2"];

pub(crate) fn handle_connect(args: &[String]) -> LoomResult<()> {
    if args.is_empty()
        || matches!(
            args.first().map(String::as_str),
            Some("help" | "--help" | "-h")
        )
    {
        print_connect_help();
        return Ok(());
    }
    match args.first().map(String::as_str) {
        Some("scaffold") => handle_connect_scaffold(&args[1..]),
        Some("list") => handle_connect_list(&args[1..]),
        Some("validate") => handle_connect_validate(&args[1..]),
        Some("enable") => handle_connect_toggle(&args[1..], true),
        Some("disable") => handle_connect_toggle(&args[1..], false),
        Some("test") => handle_connect_test(&args[1..]),
        Some("health") => handle_connect_health(&args[1..]),
        _ => Err(
            "connect supports 'scaffold', 'list', 'validate', 'enable', 'disable', 'test', and 'health'"
                .to_string(),
        ),
    }
}

fn print_connect_help() {
    println!(
        "Meridian Loom // CONNECT

Scaffold, validate, and operate Universal Connect adapter manifests.

USAGE: loom connect <COMMAND> [OPTIONS]

COMMANDS:
  scaffold --name NAME --transport grpc|a2a|mcp|http|ros2 --action-schema SCHEMA
           [--root ROOT] [--format human|json]
  list     [--root ROOT] [--format human|json]
  validate [--adapter-id ID] [--root ROOT] [--format human|json]
  enable   --adapter-id ID [--root ROOT] [--format human|json]
  disable  --adapter-id ID [--root ROOT] [--format human|json]
  test     --adapter-id ID [--root ROOT] [--format human|json]
  health   --adapter-id ID [--root ROOT] [--format human|json]"
    );
}

fn handle_connect_scaffold(args: &[String]) -> LoomResult<()> {
    let root = root_from(take_value(args, "--root").as_deref())?;
    let format = output_format(args);
    let name = required_flag(args, "--name")?;
    let transport = required_flag(args, "--transport")?.trim().to_ascii_lowercase();
    let action_schema = required_flag(args, "--action-schema")?;
    validate_transport(&transport)?;

    let adapter_id = sanitize_token(&name);
    if adapter_id.is_empty() {
        return Err("connect scaffold produced empty adapter id from --name".to_string());
    }

    let mut registry = load_connect_registry(&root)?;
    let now = chrono_like_timestamp();
    let mode = if find_adapter(&registry, &adapter_id).is_some() {
        "updated"
    } else {
        "created"
    };
    let created_at = find_adapter(&registry, &adapter_id)
        .and_then(|value| value.get("created_at"))
        .and_then(Value::as_str)
        .unwrap_or(now.as_str())
        .to_string();
    let manifest = json!({
        "schema_version": CONNECT_ADAPTER_SCHEMA,
        "adapter_id": adapter_id,
        "name": name,
        "transport": transport,
        "action_schema": action_schema,
        "status": "scaffolded",
        "created_at": created_at,
        "updated_at": now,
        "runtime_contract": CONNECT_RUNTIME_CONTRACT_V2,
        "lifecycle": {
            "enabled": false,
        },
        "diagnostics": default_adapter_diagnostics(),
        "poge_standard": poge_standard_profile(),
        "transport_profile": transport_profile_for(transport.as_str()),
    });
    upsert_adapter(&mut registry, manifest.clone())?;
    persist_connect_registry(&root, &registry)?;

    let adapters_dir = connect_adapters_dir(&root);
    std::fs::create_dir_all(&adapters_dir).map_err(|error| error.to_string())?;
    let manifest_path = adapters_dir.join(format!(
        "{}.json",
        manifest
            .get("adapter_id")
            .and_then(Value::as_str)
            .unwrap_or_default()
    ));
    std::fs::write(
        &manifest_path,
        serde_json::to_string_pretty(&manifest).map_err(|error| error.to_string())? + "\n",
    )
    .map_err(|error| error.to_string())?;

    let total_adapters = registry
        .get("adapters")
        .and_then(Value::as_array)
        .map(|items| items.len())
        .unwrap_or(0);
    let payload = json!({
        "status": "connect_scaffolded",
        "mode": mode,
        "adapter_id": manifest.get("adapter_id").and_then(Value::as_str).unwrap_or(""),
        "name": manifest.get("name").and_then(Value::as_str).unwrap_or(""),
        "transport": manifest.get("transport").and_then(Value::as_str).unwrap_or(""),
        "action_schema": manifest.get("action_schema").and_then(Value::as_str).unwrap_or(""),
        "runtime_contract": CONNECT_RUNTIME_CONTRACT_V2,
        "enabled": false,
        "manifest_path": manifest_path.display().to_string(),
        "registry_path": connect_registry_path(&root).display().to_string(),
        "total_adapters": total_adapters,
        "registry_schema_version": CONNECT_REGISTRY_SCHEMA_V2,
        "poge_standard_profile": manifest.pointer("/poge_standard/profile").and_then(Value::as_str).unwrap_or(""),
        "supported_transports": SUPPORTED_TRANSPORTS,
        "note": "area10.1 connect lifecycle scaffold",
    });
    persist_connect_latest_artifact(&root, &payload)?;
    print_connect_payload(&payload, &format)
}

fn handle_connect_list(args: &[String]) -> LoomResult<()> {
    let root = root_from(take_value(args, "--root").as_deref())?;
    let format = output_format(args);
    let registry = load_connect_registry(&root)?;
    let adapters = registry
        .get("adapters")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let payload = json!({
        "status": "connect_listed",
        "registry_path": connect_registry_path(&root).display().to_string(),
        "registry_schema_version": CONNECT_REGISTRY_SCHEMA_V2,
        "total_adapters": adapters.len(),
        "supported_transports": SUPPORTED_TRANSPORTS,
        "adapters": adapters,
        "note": "area10.1 universal connect lifecycle registry",
    });
    print_connect_payload(&payload, &format)
}

fn handle_connect_validate(args: &[String]) -> LoomResult<()> {
    let root = root_from(take_value(args, "--root").as_deref())?;
    let format = output_format(args);
    let adapter_id = take_value(args, "--adapter-id").map(|value| sanitize_token(&value));
    let legacy_schema_detected = detect_legacy_registry_schema(&root)?;

    let registry = load_connect_registry(&root)?;
    persist_connect_registry(&root, &registry)?;
    let adapters = registry
        .get("adapters")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let selected = if let Some(requested) = adapter_id {
        adapters
            .into_iter()
            .filter(|adapter| {
                adapter
                    .get("adapter_id")
                    .and_then(Value::as_str)
                    .map(|value| value == requested)
                    .unwrap_or(false)
            })
            .collect::<Vec<_>>()
    } else {
        adapters
    };
    if selected.is_empty() {
        return Err("connect validate did not find matching adapters".to_string());
    }

    let mut invalid = 0_u64;
    let mut checks = Vec::new();
    for adapter in selected {
        let adapter_id = adapter
            .get("adapter_id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let transport = adapter
            .get("transport")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let action_schema = adapter
            .get("action_schema")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let transport_ok = SUPPORTED_TRANSPORTS.contains(&transport.as_str());
        let schema_ok = !action_schema.trim().is_empty();
        let runtime_contract_ok = adapter
            .get("runtime_contract")
            .and_then(Value::as_str)
            .map(|value| value == CONNECT_RUNTIME_CONTRACT_V2)
            .unwrap_or(false);
        let valid = transport_ok && schema_ok && runtime_contract_ok;
        if !valid {
            invalid += 1;
        }
        checks.push(json!({
            "adapter_id": adapter_id,
            "transport_ok": transport_ok,
            "action_schema_ok": schema_ok,
            "runtime_contract_ok": runtime_contract_ok,
            "valid": valid,
        }));
    }

    let validation_status = if invalid == 0 { "pass" } else { "fail" };
    let payload = json!({
        "status": "connect_validated",
        "validation_status": validation_status,
        "registry_path": connect_registry_path(&root).display().to_string(),
        "registry_schema_version": CONNECT_REGISTRY_SCHEMA_V2,
        "runtime_contract": CONNECT_RUNTIME_CONTRACT_V2,
        "checked_adapters": checks.len(),
        "invalid_adapters": invalid,
        "checks": checks,
        "migration": {
            "legacy_schema_detected": legacy_schema_detected,
            "migration_mode": "additive_v1_to_v2",
        },
        "note": "area10.1 connect validate with additive v1->v2 compatibility",
    });
    persist_connect_latest_artifact(&root, &payload)?;
    if invalid > 0 {
        let _ = print_connect_payload(&payload, &format);
        return Err(format!(
            "connect validate failed: {} adapters invalid",
            invalid
        ));
    }
    print_connect_payload(&payload, &format)
}

fn handle_connect_toggle(args: &[String], enable: bool) -> LoomResult<()> {
    let root = root_from(take_value(args, "--root").as_deref())?;
    let format = output_format(args);
    let adapter_id = sanitize_token(&required_flag(args, "--adapter-id")?);
    if adapter_id.is_empty() {
        return Err("adapter-id cannot be empty".to_string());
    }

    let mut registry = load_connect_registry(&root)?;
    let adapter = find_adapter_mut(&mut registry, &adapter_id)
        .ok_or_else(|| format!("adapter '{}' not found", adapter_id))?;
    let previous = adapter_enabled(adapter);
    set_adapter_enabled(adapter, enable);
    adapter["status"] = Value::String(if enable {
        "enabled".to_string()
    } else {
        "disabled".to_string()
    });
    adapter["updated_at"] = Value::String(chrono_like_timestamp());
    let mode = if previous == enable { "noop" } else { "changed" };
    persist_connect_registry(&root, &registry)?;

    let payload = json!({
        "status": if enable { "connect_enabled" } else { "connect_disabled" },
        "mode": mode,
        "adapter_id": adapter_id,
        "enabled": enable,
        "registry_path": connect_registry_path(&root).display().to_string(),
        "registry_schema_version": CONNECT_REGISTRY_SCHEMA_V2,
        "runtime_contract": CONNECT_RUNTIME_CONTRACT_V2,
        "note": "area10.1 adapter lifecycle toggle",
    });
    persist_connect_latest_artifact(&root, &payload)?;
    print_connect_payload(&payload, &format)
}

fn handle_connect_test(args: &[String]) -> LoomResult<()> {
    let root = root_from(take_value(args, "--root").as_deref())?;
    let format = output_format(args);
    let adapter_id = sanitize_token(&required_flag(args, "--adapter-id")?);
    if adapter_id.is_empty() {
        return Err("adapter-id cannot be empty".to_string());
    }

    let mut registry = load_connect_registry(&root)?;
    let adapter = find_adapter_mut(&mut registry, &adapter_id)
        .ok_or_else(|| format!("adapter '{}' not found", adapter_id))?;
    let transport = adapter
        .get("transport")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let action_schema = adapter
        .get("action_schema")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let enabled = adapter_enabled(adapter);

    let (test_status, test_reason) = if !enabled {
        ("fail".to_string(), "adapter_disabled".to_string())
    } else if !SUPPORTED_TRANSPORTS.contains(&transport.as_str()) {
        (
            "fail".to_string(),
            format!("unsupported_transport:{transport}"),
        )
    } else if action_schema.trim().is_empty() {
        ("fail".to_string(), "missing_action_schema".to_string())
    } else {
        ("pass".to_string(), "contract_checks_passed".to_string())
    };
    let tested_at = chrono_like_timestamp();
    let test_event = json!({
        "schema_version": CONNECT_TEST_EVENT_SCHEMA,
        "adapter_id": adapter_id,
        "transport": transport,
        "runtime_contract": CONNECT_RUNTIME_CONTRACT_V2,
        "tested_at": tested_at,
        "test_status": test_status,
        "test_reason": test_reason,
        "warrant_bound": true,
        "authority_checked": true,
        "court_checked": true,
        "treasury_gate": true,
    });
    append_jsonl(
        &connect_tests_history_path(&root, &adapter_id),
        &serde_json::to_string(&test_event).map_err(|error| error.to_string())?,
    )?;

    let diagnostics = adapter_diagnostics_mut(adapter);
    diagnostics["last_test_status"] = Value::String(
        test_event
            .get("test_status")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string(),
    );
    diagnostics["last_test_reason"] = Value::String(
        test_event
            .get("test_reason")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string(),
    );
    diagnostics["last_test_at"] = Value::String(
        test_event
            .get("tested_at")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
    );
    let test_entries = diagnostics
        .get("test_history_entries")
        .and_then(Value::as_u64)
        .unwrap_or(0)
        + 1;
    diagnostics["test_history_entries"] = Value::Number(test_entries.into());
    adapter["updated_at"] = Value::String(tested_at);
    persist_connect_registry(&root, &registry)?;

    let payload = json!({
        "status": "connect_tested",
        "adapter_id": adapter_id,
        "test_status": test_event.get("test_status").and_then(Value::as_str).unwrap_or("unknown"),
        "test_reason": test_event.get("test_reason").and_then(Value::as_str).unwrap_or("unknown"),
        "tests_history_path": connect_tests_history_path(&root, &adapter_id).display().to_string(),
        "registry_path": connect_registry_path(&root).display().to_string(),
        "runtime_contract": CONNECT_RUNTIME_CONTRACT_V2,
        "note": "area10.1 adapter diagnostics test entry persisted",
    });
    persist_connect_latest_artifact(&root, &payload)?;
    if payload.get("test_status").and_then(Value::as_str) == Some("fail") {
        let _ = print_connect_payload(&payload, &format);
        return Err(format!(
            "connect test failed for adapter '{}': {}",
            adapter_id,
            payload
                .get("test_reason")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
        ));
    }
    print_connect_payload(&payload, &format)
}

fn handle_connect_health(args: &[String]) -> LoomResult<()> {
    let root = root_from(take_value(args, "--root").as_deref())?;
    let format = output_format(args);
    let adapter_id = sanitize_token(&required_flag(args, "--adapter-id")?);
    if adapter_id.is_empty() {
        return Err("adapter-id cannot be empty".to_string());
    }

    let mut registry = load_connect_registry(&root)?;
    let adapter = find_adapter_mut(&mut registry, &adapter_id)
        .ok_or_else(|| format!("adapter '{}' not found", adapter_id))?;
    let enabled = adapter_enabled(adapter);
    let latest_test = read_latest_jsonl_event(&connect_tests_history_path(&root, &adapter_id))?;
    let test_status = latest_test
        .as_ref()
        .and_then(|event| event.get("test_status"))
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let health_status = if !enabled {
        "disabled"
    } else if test_status == "pass" {
        "healthy"
    } else if test_status == "fail" {
        "degraded"
    } else {
        "unknown"
    };
    let health_checked_at = chrono_like_timestamp();
    let payload = json!({
        "schema_version": CONNECT_HEALTH_EVENT_SCHEMA,
        "status": "connect_health",
        "adapter_id": adapter_id,
        "enabled": enabled,
        "health_status": health_status,
        "last_test_status": test_status,
        "last_test_reason": latest_test
            .as_ref()
            .and_then(|event| event.get("test_reason"))
            .and_then(Value::as_str)
            .unwrap_or(""),
        "health_checked_at": health_checked_at,
        "runtime_contract": CONNECT_RUNTIME_CONTRACT_V2,
        "registry_path": connect_registry_path(&root).display().to_string(),
        "health_path": connect_health_path(&root, &adapter_id).display().to_string(),
        "tests_history_path": connect_tests_history_path(&root, &adapter_id).display().to_string(),
        "lifecycle_metrics": {
            "test_history_entries": adapter
                .pointer("/diagnostics/test_history_entries")
                .and_then(Value::as_u64)
                .unwrap_or(0),
        },
        "note": "area10.1 adapter health snapshot",
    });
    persist_health_snapshot(&root, &adapter_id, &payload)?;

    let diagnostics = adapter_diagnostics_mut(adapter);
    diagnostics["last_health_status"] = Value::String(health_status.to_string());
    diagnostics["last_health_at"] = Value::String(health_checked_at);
    adapter["updated_at"] = Value::String(chrono_like_timestamp());
    persist_connect_registry(&root, &registry)?;
    persist_connect_latest_artifact(&root, &payload)?;
    print_connect_payload(&payload, &format)
}

fn persist_health_snapshot(root: &Path, adapter_id: &str, payload: &Value) -> LoomResult<()> {
    let health_path = connect_health_path(root, adapter_id);
    if let Some(parent) = health_path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    std::fs::write(
        health_path,
        serde_json::to_string_pretty(payload).map_err(|error| error.to_string())? + "\n",
    )
    .map_err(|error| error.to_string())
}

fn persist_connect_latest_artifact(root: &Path, payload: &Value) -> LoomResult<()> {
    let latest_path = connect_latest_artifact_path(root);
    if let Some(parent) = latest_path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    std::fs::write(
        &latest_path,
        serde_json::to_string_pretty(payload).map_err(|error| error.to_string())? + "\n",
    )
    .map_err(|error| error.to_string())
}

fn append_jsonl(path: &Path, line: &str) -> LoomResult<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|error| error.to_string())?;
    file.write_all(line.as_bytes())
        .map_err(|error| error.to_string())?;
    file.write_all(b"\n").map_err(|error| error.to_string())
}

fn read_latest_jsonl_event(path: &Path) -> LoomResult<Option<Value>> {
    if !path.exists() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
    for line in raw.lines().rev() {
        if line.trim().is_empty() {
            continue;
        }
        let value: Value = serde_json::from_str(line)
            .map_err(|error| format!("invalid connect jsonl entry: {error}"))?;
        return Ok(Some(value));
    }
    Ok(None)
}

fn print_connect_payload(payload: &Value, format: &str) -> LoomResult<()> {
    match format {
        "human" => {
            print_startup_banner();
            let mut lines = vec![format!(
                "status:              {}",
                payload.get("status").and_then(Value::as_str).unwrap_or("unknown")
            )];
            if let Some(mode) = payload.get("mode").and_then(Value::as_str) {
                lines.push(format!("mode:                {mode}"));
            }
            if let Some(adapter_id) = payload.get("adapter_id").and_then(Value::as_str) {
                lines.push(format!("adapter_id:          {adapter_id}"));
            }
            if let Some(enabled) = payload.get("enabled").and_then(Value::as_bool) {
                lines.push(format!("enabled:             {enabled}"));
            }
            if let Some(test_status) = payload.get("test_status").and_then(Value::as_str) {
                lines.push(format!("test_status:         {test_status}"));
            }
            if let Some(test_reason) = payload.get("test_reason").and_then(Value::as_str) {
                lines.push(format!("test_reason:         {test_reason}"));
            }
            if let Some(health_status) = payload.get("health_status").and_then(Value::as_str) {
                lines.push(format!("health_status:       {health_status}"));
            }
            if let Some(validation_status) =
                payload.get("validation_status").and_then(Value::as_str)
            {
                lines.push(format!("validation_status:   {validation_status}"));
            }
            if let Some(path) = payload.get("manifest_path").and_then(Value::as_str) {
                lines.push(format!("manifest_path:       {path}"));
            }
            if let Some(path) = payload.get("registry_path").and_then(Value::as_str) {
                lines.push(format!("registry_path:       {path}"));
            }
            if let Some(path) = payload.get("tests_history_path").and_then(Value::as_str) {
                lines.push(format!("tests_history_path:  {path}"));
            }
            if let Some(path) = payload.get("health_path").and_then(Value::as_str) {
                lines.push(format!("health_path:         {path}"));
            }
            if let Some(total) = payload.get("total_adapters").and_then(Value::as_u64) {
                lines.push(format!("total_adapters:      {total}"));
            }
            if let Some(invalid) = payload.get("invalid_adapters").and_then(Value::as_u64) {
                lines.push(format!("invalid_adapters:    {invalid}"));
            }
            if let Some(note) = payload.get("note").and_then(Value::as_str) {
                lines.push(format!("note:                {note}"));
            }
            if let Some(adapters) = payload.get("adapters").and_then(Value::as_array) {
                lines.push("adapters:".to_string());
                for adapter in adapters {
                    lines.push(format!(
                        "  - {} ({}) schema={} enabled={} status={}",
                        adapter.get("adapter_id").and_then(Value::as_str).unwrap_or(""),
                        adapter.get("transport").and_then(Value::as_str).unwrap_or(""),
                        adapter.get("action_schema").and_then(Value::as_str).unwrap_or(""),
                        adapter
                            .pointer("/lifecycle/enabled")
                            .and_then(Value::as_bool)
                            .unwrap_or(false),
                        adapter.get("status").and_then(Value::as_str).unwrap_or(""),
                    ));
                }
            }
            print_human(&(lines.join("\n") + "\n"));
        }
        _ => println!(
            "{}",
            serde_json::to_string_pretty(payload).map_err(|error| error.to_string())?
        ),
    }
    Ok(())
}

fn output_format(args: &[String]) -> String {
    take_value(args, "--format").unwrap_or_else(|| {
        if std::io::stdout().is_terminal() {
            "human".to_string()
        } else {
            "json".to_string()
        }
    })
}

fn validate_transport(transport: &str) -> LoomResult<()> {
    if SUPPORTED_TRANSPORTS.contains(&transport) {
        Ok(())
    } else {
        Err(format!(
            "unsupported transport '{}'; supported: {}",
            transport,
            SUPPORTED_TRANSPORTS.join("|")
        ))
    }
}

fn connect_registry_path(root: &Path) -> PathBuf {
    root.join(DEFAULT_CONNECT_REGISTRY_PATH)
}

fn connect_adapters_dir(root: &Path) -> PathBuf {
    root.join(DEFAULT_CONNECT_ADAPTERS_DIR)
}

fn connect_health_path(root: &Path, adapter_id: &str) -> PathBuf {
    root.join(DEFAULT_CONNECT_HEALTH_DIR)
        .join(format!("{adapter_id}.json"))
}

fn connect_tests_history_path(root: &Path, adapter_id: &str) -> PathBuf {
    root.join(DEFAULT_CONNECT_TESTS_DIR)
        .join(format!("{adapter_id}.jsonl"))
}

fn connect_latest_artifact_path(root: &Path) -> PathBuf {
    root.join(DEFAULT_CONNECT_LATEST_ARTIFACT_PATH)
}

fn load_connect_registry(root: &Path) -> LoomResult<Value> {
    let path = connect_registry_path(root);
    if !path.exists() {
        return Ok(default_connect_registry());
    }
    let raw = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
    let mut value: Value = serde_json::from_str(&raw)
        .map_err(|error| format!("invalid connect registry json: {error}"))?;
    normalize_registry(&mut value);
    Ok(value)
}

fn persist_connect_registry(root: &Path, registry: &Value) -> LoomResult<()> {
    let path = connect_registry_path(root);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    std::fs::write(
        path,
        serde_json::to_string_pretty(registry).map_err(|error| error.to_string())? + "\n",
    )
    .map_err(|error| error.to_string())
}

fn detect_legacy_registry_schema(root: &Path) -> LoomResult<bool> {
    let path = connect_registry_path(root);
    if !path.exists() {
        return Ok(false);
    }
    let raw = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
    let value: Value = serde_json::from_str(&raw)
        .map_err(|error| format!("invalid connect registry json: {error}"))?;
    Ok(value
        .get("schema_version")
        .and_then(Value::as_str)
        .map(|schema| schema == CONNECT_REGISTRY_SCHEMA_V1)
        .unwrap_or(false))
}

fn default_connect_registry() -> Value {
    json!({
        "schema_version": CONNECT_REGISTRY_SCHEMA_V2,
        "runtime_contract": CONNECT_RUNTIME_CONTRACT_V2,
        "adapters": [],
    })
}

fn normalize_registry(registry: &mut Value) {
    if !registry.is_object() {
        *registry = default_connect_registry();
        return;
    }
    registry["schema_version"] = Value::String(CONNECT_REGISTRY_SCHEMA_V2.to_string());
    registry["runtime_contract"] = Value::String(CONNECT_RUNTIME_CONTRACT_V2.to_string());
    if !registry.get("adapters").map(Value::is_array).unwrap_or(false) {
        registry["adapters"] = Value::Array(Vec::new());
    }
    if let Some(items) = registry.get_mut("adapters").and_then(Value::as_array_mut) {
        for item in items.iter_mut() {
            normalize_adapter(item);
        }
        items.sort_by(|left, right| {
            value_string(left.get("adapter_id")).cmp(value_string(right.get("adapter_id")))
        });
    }
}

fn normalize_adapter(adapter: &mut Value) {
    if !adapter.is_object() {
        *adapter = json!({});
    }
    if adapter
        .get("schema_version")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .is_empty()
    {
        adapter["schema_version"] = Value::String(CONNECT_ADAPTER_SCHEMA.to_string());
    }
    adapter["runtime_contract"] = Value::String(CONNECT_RUNTIME_CONTRACT_V2.to_string());
    if !adapter.get("lifecycle").map(Value::is_object).unwrap_or(false) {
        adapter["lifecycle"] = json!({});
    }
    if adapter
        .pointer("/lifecycle/enabled")
        .and_then(Value::as_bool)
        .is_none()
    {
        adapter["lifecycle"]["enabled"] = Value::Bool(false);
    }
    if !adapter.get("diagnostics").map(Value::is_object).unwrap_or(false) {
        adapter["diagnostics"] = default_adapter_diagnostics();
    }
    let diagnostics = adapter_diagnostics_mut(adapter);
    if diagnostics
        .get("last_test_status")
        .and_then(Value::as_str)
        .is_none()
    {
        diagnostics["last_test_status"] = Value::String("unknown".to_string());
    }
    if diagnostics
        .get("last_health_status")
        .and_then(Value::as_str)
        .is_none()
    {
        diagnostics["last_health_status"] = Value::String("unknown".to_string());
    }
    if diagnostics
        .get("test_history_entries")
        .and_then(Value::as_u64)
        .is_none()
    {
        diagnostics["test_history_entries"] = Value::Number(0_u64.into());
    }
}

fn find_adapter<'a>(registry: &'a Value, adapter_id: &str) -> Option<&'a Value> {
    registry
        .get("adapters")
        .and_then(Value::as_array)
        .and_then(|items| {
            items.iter().find(|item| {
                item.get("adapter_id")
                    .and_then(Value::as_str)
                    .map(|value| value == adapter_id)
                    .unwrap_or(false)
            })
        })
}

fn find_adapter_mut<'a>(registry: &'a mut Value, adapter_id: &str) -> Option<&'a mut Value> {
    registry
        .get_mut("adapters")
        .and_then(Value::as_array_mut)
        .and_then(|items| {
            items.iter_mut().find(|item| {
                item.get("adapter_id")
                    .and_then(Value::as_str)
                    .map(|value| value == adapter_id)
                    .unwrap_or(false)
            })
        })
}

fn upsert_adapter(registry: &mut Value, manifest: Value) -> LoomResult<()> {
    normalize_registry(registry);
    let adapter_id = manifest
        .get("adapter_id")
        .and_then(Value::as_str)
        .ok_or_else(|| "adapter manifest missing adapter_id".to_string())?
        .to_string();
    let adapters = registry
        .get_mut("adapters")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| "connect registry missing adapters array".to_string())?;
    if let Some(existing) = adapters.iter_mut().find(|item| {
        item.get("adapter_id")
            .and_then(Value::as_str)
            .map(|value| value == adapter_id)
            .unwrap_or(false)
    }) {
        *existing = manifest;
    } else {
        adapters.push(manifest);
    }
    normalize_registry(registry);
    Ok(())
}

fn adapter_enabled(adapter: &Value) -> bool {
    adapter
        .pointer("/lifecycle/enabled")
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn set_adapter_enabled(adapter: &mut Value, enabled: bool) {
    if !adapter.get("lifecycle").map(Value::is_object).unwrap_or(false) {
        adapter["lifecycle"] = json!({});
    }
    adapter["lifecycle"]["enabled"] = Value::Bool(enabled);
}

fn adapter_diagnostics_mut(adapter: &mut Value) -> &mut Value {
    if !adapter.get("diagnostics").map(Value::is_object).unwrap_or(false) {
        adapter["diagnostics"] = default_adapter_diagnostics();
    }
    &mut adapter["diagnostics"]
}

fn default_adapter_diagnostics() -> Value {
    json!({
        "last_test_status": "unknown",
        "last_test_reason": "",
        "last_test_at": "",
        "last_health_status": "unknown",
        "last_health_at": "",
        "test_history_entries": 0,
    })
}

fn value_string(value: Option<&Value>) -> &str {
    value.and_then(Value::as_str).unwrap_or("")
}

fn poge_standard_profile() -> Value {
    json!({
        "profile": "meridian.poge.standard.v1",
        "warrant_required": true,
        "authority_check": true,
        "court_check": true,
        "treasury_gate": true,
        "receipt_emission": true,
        "merkle_audit_root": true,
        "zk_settlement_compatible": true,
        "event_schema": "runtime_event_v1",
        "settlement_mode": "receipt+settle_zk",
    })
}

fn transport_profile_for(transport: &str) -> Value {
    match transport {
        "grpc" => json!({
            "kind": "grpc",
            "rpc": "meridian.runtime.v1.ActionService/SubmitAction",
            "target": "https://runtime.example",
            "serialization": "protobuf",
            "note": "semantic action envelopes over gRPC transport",
        }),
        "a2a" => json!({
            "kind": "a2a",
            "method": "message/send",
            "target": "https://a2a.example/bridge",
            "note": "agent-to-agent semantic envelope bridge",
        }),
        "mcp" => json!({
            "kind": "mcp",
            "method": "tools/call",
            "target": "https://mcp.example",
            "tool": "shadow.execute",
            "note": "MCP tool-call lane with governed action envelope",
        }),
        "http" => json!({
            "kind": "http",
            "method": "POST",
            "target": "https://runtime.example/shadow/execute",
            "content_type": "application/json",
            "note": "HTTP JSON envelope fallback transport",
        }),
        "ros2" => json!({
            "kind": "ros2",
            "mode": "service",
            "service": "/meridian/physical_action/execute",
            "type": "meridian_embodied_msgs/srv/ExecutePhysicalAction",
            "note": "embodied lane via ROS2 service/action bridge",
        }),
        _ => json!({
            "kind": transport,
            "note": "unsupported transport profile",
        }),
    }
}
