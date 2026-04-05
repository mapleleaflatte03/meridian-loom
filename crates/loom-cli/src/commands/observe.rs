use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use crate::*;
use serde_json::{json, Value};

const OBSERVABILITY_CONTRACT_VERSION: &str = "observability_contract_v1";
const OBSERVABILITY_CONTRACT_PATH: &str = "state/observability/observability_contract_v1.json";
const OBSERVABILITY_LATEST_ARTIFACT_PATH: &str = "artifacts/observability/latest.json";
const ROUTE_TRACE_PATH: &str = "state/gateway/route_decision_trace.jsonl";

pub(crate) fn handle_observe(args: &[String]) -> LoomResult<()> {
    if args.is_empty()
        || matches!(
            args.first().map(String::as_str),
            Some("help" | "--help" | "-h")
        )
    {
        print_observe_help();
        return Ok(());
    }
    match args.first().map(String::as_str) {
        Some("summary") => handle_observe_summary(&args[1..]),
        Some("alerts") => handle_observe_alerts(&args[1..]),
        Some("watch") => handle_observe_watch(&args[1..]),
        _ => Err("observe supports 'summary', 'alerts', and 'watch'".to_string()),
    }
}

fn print_observe_help() {
    println!(
        "Meridian Loom // OBSERVE

Operator-grade observability summary for queue, service, proof chain, and route drift.

USAGE: loom observe <COMMAND> [OPTIONS]

COMMANDS:
  summary [--root ROOT] [--fix-hints] [--format human|json]
  alerts  [--root ROOT] [--fix-hints] [--format human|json]
  watch   [--root ROOT] [--iterations N] [--interval-seconds N] [--fix-hints] [--format human|json]"
    );
}

fn handle_observe_summary(args: &[String]) -> LoomResult<()> {
    let root = root_from(take_value(args, "--root").as_deref())?;
    let format = output_format(args);
    let include_fix_hints = has_flag(args, "--fix-hints");
    let payload = build_observability_payload(&root, include_fix_hints)?;
    persist_observability_payload(&root, &payload)?;
    print_observe_payload(&payload, &format)
}

fn handle_observe_alerts(args: &[String]) -> LoomResult<()> {
    let root = root_from(take_value(args, "--root").as_deref())?;
    let format = output_format(args);
    let include_fix_hints = has_flag(args, "--fix-hints");
    let payload = build_observability_payload(&root, include_fix_hints)?;
    let alerts_payload = json!({
        "status": "observe_alerts",
        "contract_version": OBSERVABILITY_CONTRACT_VERSION,
        "overall_status": payload.get("overall_status").cloned().unwrap_or(Value::String("unknown".to_string())),
        "alert_count": payload.get("alerts").and_then(Value::as_array).map(|items| items.len()).unwrap_or(0),
        "alerts": payload.get("alerts").cloned().unwrap_or_else(|| Value::Array(vec![])),
        "fix_hints": payload.get("fix_hints").cloned().unwrap_or_else(|| Value::Array(vec![])),
        "paths": payload.get("paths").cloned().unwrap_or(Value::Null),
    });
    print_observe_payload(&alerts_payload, &format)
}

fn handle_observe_watch(args: &[String]) -> LoomResult<()> {
    let root = root_from(take_value(args, "--root").as_deref())?;
    let format = output_format(args);
    let include_fix_hints = has_flag(args, "--fix-hints");
    let iterations = take_value(args, "--iterations")
        .and_then(|raw| raw.parse::<usize>().ok())
        .unwrap_or(5)
        .max(1);
    let interval_seconds = take_value(args, "--interval-seconds")
        .and_then(|raw| raw.parse::<u64>().ok())
        .unwrap_or(1)
        .max(1);

    let mut frames = Vec::new();
    for index in 0..iterations {
        let payload = build_observability_payload(&root, include_fix_hints)?;
        if format == "human" {
            print_observe_payload(&payload, "human")?;
        }
        frames.push(json!({
            "iteration": index + 1,
            "observed_at": chrono_like_timestamp(),
            "overall_status": payload.get("overall_status").cloned().unwrap_or(Value::String("unknown".to_string())),
            "alerts": payload.get("alerts").cloned().unwrap_or_else(|| Value::Array(vec![])),
            "components": payload.get("components").cloned().unwrap_or(Value::Null),
        }));
        if index + 1 < iterations {
            thread::sleep(Duration::from_secs(interval_seconds));
        }
    }
    let payload = json!({
        "status": "observe_watch",
        "contract_version": OBSERVABILITY_CONTRACT_VERSION,
        "iterations": iterations,
        "interval_seconds": interval_seconds,
        "frames": frames,
    });
    if format == "human" {
        Ok(())
    } else {
        print_observe_payload(&payload, "json")
    }
}

fn build_observability_payload(root: &Path, include_fix_hints: bool) -> LoomResult<Value> {
    let service = runtime_service_status(root, None).map_err(|error| error.to_string())?;
    let queue = queue_status(root).map_err(|error| error.to_string())?;
    let proof_chain = proof_chain_snapshot(root)?;
    let route_trace = route_trace_snapshot(root)?;

    let mut alerts = Vec::new();
    if !service.running {
        alerts.push(alert(
            "service_not_running",
            "warning",
            "runtime service is not currently running",
        ));
    }
    if queue.total_pending > 0 {
        alerts.push(alert(
            "queue_backlog",
            "warning",
            &format!("queue has {} pending jobs", queue.total_pending),
        ));
    }
    if proof_chain
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("missing")
        != "ready"
    {
        alerts.push(alert(
            "proof_chain_missing",
            "critical",
            "zk + settlement latest artifacts are not both available",
        ));
    }
    if route_trace
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("missing")
        == "missing"
    {
        alerts.push(alert(
            "route_trace_missing",
            "warning",
            "route decision trace is not yet available",
        ));
    }
    if route_trace
        .get("drift_count")
        .and_then(Value::as_u64)
        .unwrap_or(0)
        > 0
    {
        alerts.push(alert(
            "route_drift_detected",
            "warning",
            "detected drift markers in route decision trace",
        ));
    }

    let overall_status = if alerts
        .iter()
        .any(|item| item.get("severity").and_then(Value::as_str) == Some("critical"))
    {
        "degraded"
    } else if alerts.is_empty() {
        "healthy"
    } else {
        "warning"
    };

    let mut payload = json!({
        "status": "observe_summary",
        "contract_version": OBSERVABILITY_CONTRACT_VERSION,
        "generated_at": chrono_like_timestamp(),
        "overall_status": overall_status,
        "components": {
            "runtime_service": {
                "status": if service.running { "running" } else { "stopped" },
                "runtime_state_status": service.status,
                "running": service.running,
                "pending_jobs": service.pending_jobs,
                "processed_jobs": service.processed_jobs,
                "failed_jobs": service.failed_jobs,
                "requests_received": service.requests_received,
                "note": service.note,
                "state_path": service.runtime_state_path.display().to_string(),
                "metrics_path": service.metrics_path.display().to_string(),
            },
            "queue": {
                "pending_records": queue.pending_records,
                "acked_records": queue.acked_records,
                "total_pending": queue.total_pending,
                "standard_depth": queue.standard_depth,
                "privileged_depth": queue.privileged_depth,
                "budget_heavy_depth": queue.budget_heavy_depth,
                "sanction_sensitive_depth": queue.sanction_sensitive_depth,
                "queue_dir": queue.queue_dir.display().to_string(),
            },
            "proof_chain": proof_chain,
            "route_decision": route_trace,
        },
        "alerts": alerts,
        "fix_hints": Value::Array(Vec::new()),
        "paths": {
            "contract": observability_contract_path(root).display().to_string(),
            "latest_artifact": observability_latest_artifact_path(root).display().to_string(),
        }
    });

    if include_fix_hints {
        payload["fix_hints"] = Value::Array(fix_hints_from_alerts(
            payload
                .get("alerts")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default(),
        ));
    }
    Ok(payload)
}

fn proof_chain_snapshot(root: &Path) -> LoomResult<Value> {
    let zk_path = root.join("artifacts/zk/latest.json");
    let settlement_path = root.join("artifacts/settlement/latest.json");
    let zk_value = read_optional_json(&zk_path)?;
    let settlement_value = read_optional_json(&settlement_path)?;
    let zk_status = zk_value
        .as_ref()
        .and_then(|value| value.get("status"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let settlement_status = settlement_value
        .as_ref()
        .and_then(|value| value.get("status"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let status = if !zk_status.is_empty() && !settlement_status.is_empty() {
        "ready"
    } else {
        "missing"
    };
    Ok(json!({
        "status": status,
        "zk_status": zk_status,
        "settlement_status": settlement_status,
        "zk_path": zk_path.display().to_string(),
        "settlement_path": settlement_path.display().to_string(),
    }))
}

fn route_trace_snapshot(root: &Path) -> LoomResult<Value> {
    let path = root.join(ROUTE_TRACE_PATH);
    if !path.exists() {
        return Ok(json!({
            "status": "missing",
            "trace_path": path.display().to_string(),
            "trace_count": 0,
            "drift_count": 0,
            "last_trace_id": "",
        }));
    }
    let raw = std::fs::read_to_string(&path).map_err(|error| error.to_string())?;
    let mut trace_count = 0usize;
    let mut drift_count = 0usize;
    let mut last_trace_id = String::new();
    for line in raw.lines() {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        trace_count += 1;
        if value
            .get("drift_flag")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            drift_count += 1;
        }
        if let Some(trace_id) = value.get("trace_id").and_then(Value::as_str) {
            if !trace_id.trim().is_empty() {
                last_trace_id = trace_id.to_string();
            }
        }
    }
    Ok(json!({
        "status": if trace_count > 0 { "ready" } else { "empty" },
        "trace_path": path.display().to_string(),
        "trace_count": trace_count,
        "drift_count": drift_count,
        "last_trace_id": last_trace_id,
    }))
}

fn fix_hints_from_alerts(alerts: Vec<Value>) -> Vec<Value> {
    let mut hints = Vec::new();
    for item in alerts {
        let Some(code) = item.get("code").and_then(Value::as_str) else {
            continue;
        };
        let hint = match code {
            "service_not_running" => {
                "run `loom service start --root <root> --kernel-path <kernel> --service-token <token>`"
            }
            "queue_backlog" => "run `loom queue run-until-empty --root <root>` or `loom swarm run --settle-zk`",
            "proof_chain_missing" => {
                "run `loom shadow run ...` then `loom job settle --zk ...` to emit zk + settlement artifacts"
            }
            "route_trace_missing" => "enable manager routing flow and ensure route decision trace is persisted",
            "route_drift_detected" => {
                "review route decision thresholds, then rerun `loom observe summary --fix-hints`"
            }
            _ => continue,
        };
        hints.push(json!({
            "code": code,
            "hint": hint,
        }));
    }
    hints
}

fn alert(code: &str, severity: &str, message: &str) -> Value {
    json!({
        "code": code,
        "severity": severity,
        "message": message,
    })
}

fn read_optional_json(path: &Path) -> LoomResult<Option<Value>> {
    if !path.exists() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
    let value = serde_json::from_str::<Value>(&raw)
        .map_err(|error| format!("failed to parse json at {}: {}", path.display(), error))?;
    Ok(Some(value))
}

fn print_observe_payload(payload: &Value, format: &str) -> LoomResult<()> {
    match format {
        "human" => {
            print_startup_banner();
            let mut lines = vec![
                format!(
                    "status:              {}",
                    payload
                        .get("status")
                        .and_then(Value::as_str)
                        .unwrap_or("unknown")
                ),
                format!(
                    "contract_version:    {}",
                    payload
                        .get("contract_version")
                        .and_then(Value::as_str)
                        .unwrap_or(OBSERVABILITY_CONTRACT_VERSION)
                ),
                format!(
                    "overall_status:      {}",
                    payload
                        .get("overall_status")
                        .and_then(Value::as_str)
                        .unwrap_or("unknown")
                ),
            ];
            if let Some(alerts) = payload.get("alerts").and_then(Value::as_array) {
                lines.push(format!("alert_count:         {}", alerts.len()));
            }
            lines.push(String::new());
            print_human(&(lines.join("\n") + "\n"));
            Ok(())
        }
        _ => {
            print!(
                "{}",
                serde_json::to_string_pretty(payload).map_err(|error| error.to_string())?
            );
            println!();
            Ok(())
        }
    }
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

fn persist_observability_payload(root: &Path, payload: &Value) -> LoomResult<()> {
    let contract_path = observability_contract_path(root);
    if let Some(parent) = contract_path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    std::fs::write(
        &contract_path,
        serde_json::to_string_pretty(payload).map_err(|error| error.to_string())? + "\n",
    )
    .map_err(|error| error.to_string())?;

    let latest_path = observability_latest_artifact_path(root);
    if let Some(parent) = latest_path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    std::fs::write(
        &latest_path,
        serde_json::to_string_pretty(payload).map_err(|error| error.to_string())? + "\n",
    )
    .map_err(|error| error.to_string())
}

fn observability_contract_path(root: &Path) -> PathBuf {
    root.join(OBSERVABILITY_CONTRACT_PATH)
}

fn observability_latest_artifact_path(root: &Path) -> PathBuf {
    root.join(OBSERVABILITY_LATEST_ARTIFACT_PATH)
}
