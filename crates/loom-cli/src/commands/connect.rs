use std::io::IsTerminal;
use std::path::{Path, PathBuf};

use crate::*;
use serde_json::{json, Value};

const DEFAULT_CONNECT_REGISTRY_PATH: &str = "state/connect/registry.json";
const DEFAULT_CONNECT_ADAPTERS_DIR: &str = "state/connect/adapters";
const DEFAULT_CONNECT_LATEST_ARTIFACT_PATH: &str = "artifacts/connect/latest.json";
const CONNECT_REGISTRY_SCHEMA: &str = "meridian.connect.registry.v1";
const CONNECT_ADAPTER_SCHEMA: &str = "meridian.connect.adapter.v1";
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
        _ => Err("connect supports 'scaffold' and 'list'".to_string()),
    }
}

fn print_connect_help() {
    println!(
        "Meridian Loom // CONNECT

Scaffold and inspect Universal Connect adapter manifests.

USAGE: loom connect <COMMAND> [OPTIONS]

COMMANDS:
  scaffold --name NAME --transport grpc|a2a|mcp|http|ros2 --action-schema SCHEMA
           [--root ROOT] [--format human|json]
  list     [--root ROOT] [--format human|json]"
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

    let latest_path = connect_latest_artifact_path(&root);
    if let Some(parent) = latest_path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
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
        "manifest_path": manifest_path.display().to_string(),
        "registry_path": connect_registry_path(&root).display().to_string(),
        "total_adapters": total_adapters,
        "poge_standard_profile": manifest.pointer("/poge_standard/profile").and_then(Value::as_str).unwrap_or(""),
        "supported_transports": SUPPORTED_TRANSPORTS,
        "note": "direction8.1 scaffold-only adapter manifest lane",
    });
    std::fs::write(
        &latest_path,
        serde_json::to_string_pretty(&payload).map_err(|error| error.to_string())? + "\n",
    )
    .map_err(|error| error.to_string())?;
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
        "total_adapters": adapters.len(),
        "supported_transports": SUPPORTED_TRANSPORTS,
        "adapters": adapters,
        "note": "direction8.1 universal connect scaffold registry",
    });
    print_connect_payload(&payload, &format)
}

fn print_connect_payload(payload: &Value, format: &str) -> LoomResult<()> {
    match format {
        "human" => {
            print_startup_banner();
            if payload.get("status").and_then(Value::as_str) == Some("connect_scaffolded") {
                print_human(&format!(
                    "status:              {}\nmode:                {}\nadapter_id:          {}\nname:                {}\ntransport:           {}\naction_schema:       {}\nmanifest_path:       {}\nregistry_path:       {}\ntotal_adapters:      {}\nnote:                {}\n",
                    payload.get("status").and_then(Value::as_str).unwrap_or("unknown"),
                    payload.get("mode").and_then(Value::as_str).unwrap_or("unknown"),
                    payload.get("adapter_id").and_then(Value::as_str).unwrap_or(""),
                    payload.get("name").and_then(Value::as_str).unwrap_or(""),
                    payload.get("transport").and_then(Value::as_str).unwrap_or(""),
                    payload.get("action_schema").and_then(Value::as_str).unwrap_or(""),
                    payload.get("manifest_path").and_then(Value::as_str).unwrap_or(""),
                    payload.get("registry_path").and_then(Value::as_str).unwrap_or(""),
                    payload
                        .get("total_adapters")
                        .and_then(Value::as_u64)
                        .unwrap_or(0),
                    payload.get("note").and_then(Value::as_str).unwrap_or(""),
                ));
            } else {
                let mut lines = vec![
                    format!(
                        "status:              {}",
                        payload.get("status").and_then(Value::as_str).unwrap_or("unknown")
                    ),
                    format!(
                        "registry_path:       {}",
                        payload.get("registry_path").and_then(Value::as_str).unwrap_or("")
                    ),
                    format!(
                        "total_adapters:      {}",
                        payload
                            .get("total_adapters")
                            .and_then(Value::as_u64)
                            .unwrap_or(0)
                    ),
                    "adapters:".to_string(),
                ];
                if let Some(adapters) = payload.get("adapters").and_then(Value::as_array) {
                    for adapter in adapters {
                        lines.push(format!(
                            "  - {} ({}) schema={} status={}",
                            adapter.get("adapter_id").and_then(Value::as_str).unwrap_or(""),
                            adapter.get("transport").and_then(Value::as_str).unwrap_or(""),
                            adapter.get("action_schema").and_then(Value::as_str).unwrap_or(""),
                            adapter.get("status").and_then(Value::as_str).unwrap_or(""),
                        ));
                    }
                }
                print_human(&(lines.join("\n") + "\n"));
            }
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

fn default_connect_registry() -> Value {
    json!({
        "schema_version": CONNECT_REGISTRY_SCHEMA,
        "adapters": [],
    })
}

fn normalize_registry(registry: &mut Value) {
    if !registry.is_object() {
        *registry = default_connect_registry();
        return;
    }
    if registry
        .get("schema_version")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .is_empty()
    {
        registry["schema_version"] = Value::String(CONNECT_REGISTRY_SCHEMA.to_string());
    }
    if !registry.get("adapters").map(Value::is_array).unwrap_or(false) {
        registry["adapters"] = Value::Array(Vec::new());
    }
    if let Some(items) = registry.get_mut("adapters").and_then(Value::as_array_mut) {
        items.sort_by(|left, right| {
            value_string(left.get("adapter_id"))
                .cmp(value_string(right.get("adapter_id")))
        });
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
    adapters.sort_by(|left, right| {
        value_string(left.get("adapter_id")).cmp(value_string(right.get("adapter_id")))
    });
    Ok(())
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
