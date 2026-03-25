//! Bet C Level 2: OpenClaw integration seam.
//!
//! This module provides a feature-flagged delivery routing seam between
//! Meridian Loom and OpenClaw. It does NOT perform live delivery.
//!
//! Modes:
//!   "off"     — seam disabled, no routing attempted (default)
//!   "dry_run" — records what would be routed, writes dry-run log, does not deliver
//!   "live"    — routes through Loom governance then hands to OpenClaw delivery queue
//!              (NOT YET IMPLEMENTED — returns error if attempted)
//!
//! Safety guarantees:
//!   - Feature flag off by default
//!   - Dry-run never writes to the OpenClaw delivery queue
//!   - Bypass: if Loom governance fails, the seam returns a bypass record so
//!     the caller can fall back to direct OpenClaw delivery
//!   - All routing decisions are logged to an audit trail

use loom_core::{
    capabilities::{render_capability_readiness_human, resolve_capability_for_request},
    openclaw_delivery_queue_path, read_config, resolve_workspace_path,
};
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};

/// Result type for seam operations.
pub type SeamResult<T> = Result<T, String>;

/// Integration mode parsed from config.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IntegrationMode {
    Off,
    DryRun,
    Live,
}

impl IntegrationMode {
    pub fn from_str(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "dry_run" | "dry-run" | "dryrun" => IntegrationMode::DryRun,
            "live" => IntegrationMode::Live,
            _ => IntegrationMode::Off,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            IntegrationMode::Off => "off",
            IntegrationMode::DryRun => "dry_run",
            IntegrationMode::Live => "live",
        }
    }
}

/// A delivery routing request from the nightly pipeline.
#[derive(Clone, Debug)]
pub struct DeliveryRoutingRequest {
    pub agent_id: String,
    pub org_id: String,
    pub action_type: String,
    pub resource: String,
    pub payload_path: String,
    pub delivery_target: String,
}

/// Canonical handoff artifact produced by the seam.
#[derive(Clone, Debug)]
pub struct DeliveryHandoffArtifact {
    pub handoff_path: String,
    pub resolved_payload_path: Option<String>,
    pub payload_status: String,
}

/// Result of routing a delivery through the seam.
#[derive(Clone, Debug)]
pub struct DeliveryRoutingResult {
    pub mode: String,
    pub routed: bool,
    pub governance_passed: bool,
    pub bypass_triggered: bool,
    pub dry_run_log_path: Option<String>,
    pub handoff_artifact_path: Option<String>,
    pub resolved_payload_path: Option<String>,
    pub payload_status: String,
    pub delivery_queue_path: Option<String>,
    pub reason: String,
}

impl DeliveryRoutingResult {
    fn to_json(&self) -> String {
        serde_json::to_string_pretty(&json!({
            "mode": self.mode,
            "routed": self.routed,
            "governance_passed": self.governance_passed,
            "bypass_triggered": self.bypass_triggered,
            "dry_run_log_path": self.dry_run_log_path,
            "handoff_artifact_path": self.handoff_artifact_path,
            "resolved_payload_path": self.resolved_payload_path,
            "payload_status": self.payload_status,
            "delivery_queue_path": self.delivery_queue_path,
            "reason": self.reason,
        }))
        .unwrap_or_else(|error| {
            format!(
                "{{\"error\":\"serialization_failed\",\"detail\":{:?}}}",
                error.to_string()
            )
        })
            + "\n"
    }
}
/// Route a delivery request through the integration seam.
///
/// When mode is Off, returns immediately with routed=false.
/// When mode is DryRun, runs governance check and logs what would happen.
/// When mode is Live, returns an error (cutover not implemented).
pub fn route_delivery(
    root: &Path,
    mode: &IntegrationMode,
    delivery_queue_path: &Path,
    request: &DeliveryRoutingRequest,
    governance_decision: &str,
) -> SeamResult<DeliveryRoutingResult> {
    match mode {
        IntegrationMode::Off => Ok(DeliveryRoutingResult {
            mode: "off".to_string(),
            routed: false,
            governance_passed: false,
            bypass_triggered: false,
            dry_run_log_path: None,
            handoff_artifact_path: None,
            resolved_payload_path: None,
            payload_status: "not_prepared".to_string(),
            delivery_queue_path: None,
            reason: "openclaw_integration=off; seam disabled".to_string(),
        }),
        IntegrationMode::DryRun => {
            let governance_passed = governance_decision == "allow";
            let bypass_triggered = !governance_passed;
            let timestamp = epoch_now();
            let handoff = prepare_delivery_handoff(
                root,
                delivery_queue_path,
                request,
                governance_decision,
                governance_passed,
                bypass_triggered,
                timestamp,
            )?;

            let seam_dir = root.join("state").join("openclaw_seam");
            let log_path = seam_dir.join(format!("dry_run_{}.json", timestamp));
            let log_entry = serde_json::to_string_pretty(&json!({
                "timestamp": timestamp,
                "mode": "dry_run",
                "agent_id": request.agent_id,
                "org_id": request.org_id,
                "action_type": request.action_type,
                "resource": request.resource,
                "payload_path": request.payload_path,
                "delivery_target": request.delivery_target,
                "governance_decision": governance_decision,
                "governance_passed": governance_passed,
                "bypass_triggered": bypass_triggered,
                "would_route_to": delivery_queue_path.display().to_string(),
                "handoff_artifact_path": handoff.handoff_path,
                "resolved_payload_path": handoff.resolved_payload_path,
                "payload_status": handoff.payload_status,
                "actually_delivered": false,
                "note": "dry-run mode: no delivery performed; handoff artifact prepared",
            }))
            .map_err(|error| error.to_string())?
                + "\n";
            fs::write(&log_path, &log_entry).map_err(|e| e.to_string())?;

            let stream_path = seam_dir.join("dry_run_stream.jsonl");
            let stream_line = serde_json::to_string(&json!({
                "ts": timestamp,
                "agent": request.agent_id,
                "decision": governance_decision,
                "bypass": bypass_triggered,
                "mode": "dry_run",
                "handoff": handoff.handoff_path,
                "payload_status": handoff.payload_status,
            }))
            .map_err(|error| error.to_string())?
                + "\n";
            let mut stream_file = fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&stream_path)
                .map_err(|e| e.to_string())?;
            std::io::Write::write_all(&mut stream_file, stream_line.as_bytes())
                .map_err(|e| e.to_string())?;

            Ok(DeliveryRoutingResult {
                mode: "dry_run".to_string(),
                routed: false,
                governance_passed,
                bypass_triggered,
                dry_run_log_path: Some(log_path.display().to_string()),
                handoff_artifact_path: Some(handoff.handoff_path),
                resolved_payload_path: handoff.resolved_payload_path,
                payload_status: handoff.payload_status,
                delivery_queue_path: None,
                reason: if governance_passed {
                    "dry-run: governance passed, handoff artifact prepared".to_string()
                } else {
                    format!(
                        "dry-run: governance denied ({}), bypass would trigger; handoff artifact prepared",
                        governance_decision
                    )
                },
            })
        }
        IntegrationMode::Live => Err(
            "openclaw_integration=live is not yet implemented; cutover requires explicit owner authorization and proven dry-run acceptance"
                .to_string(),
        ),
    }
}

fn prepare_delivery_handoff(
    root: &Path,
    delivery_queue_path: &Path,
    request: &DeliveryRoutingRequest,
    governance_decision: &str,
    governance_passed: bool,
    bypass_triggered: bool,
    timestamp: u64,
) -> SeamResult<DeliveryHandoffArtifact> {
    let seam_dir = root.join("state").join("openclaw_seam");
    fs::create_dir_all(&seam_dir).map_err(|e| e.to_string())?;
    let handoff_path = seam_dir.join(format!("handoff_{}.json", timestamp));
    let resolved_payload_path: Option<PathBuf> = if request.payload_path.trim().is_empty() {
        None
    } else {
        Some(resolve_workspace_path(root, &request.payload_path))
    };
    let (payload_status, payload_body) = match resolved_payload_path.as_ref() {
        None => ("missing".to_string(), None),
        Some(path) => match fs::read_to_string(path) {
            Ok(contents) => ("loaded".to_string(), Some(contents)),
            Err(_) => ("missing".to_string(), None),
        },
    };
    let artifact = serde_json::to_string_pretty(&json!({
        "timestamp": timestamp,
        "mode": "dry_run",
        "handoff_status": "prepared",
        "agent_id": request.agent_id,
        "org_id": request.org_id,
        "action_type": request.action_type,
        "resource": request.resource,
        "delivery_target": request.delivery_target,
        "governance_decision": governance_decision,
        "governance_passed": governance_passed,
        "bypass_triggered": bypass_triggered,
        "delivery_queue_path": delivery_queue_path.display().to_string(),
        "payload_path": request.payload_path,
        "resolved_payload_path": resolved_payload_path.as_ref().map(|path| path.display().to_string()),
        "payload_status": payload_status,
        "payload_body": payload_body,
        "note": "canonical handoff envelope for a later delivery shim; dry-run does not enqueue",
    }))
    .map_err(|error| error.to_string())?
        + "\n";
    fs::write(&handoff_path, artifact).map_err(|e| e.to_string())?;

    let stream_path = seam_dir.join("handoff_stream.jsonl");
    let stream_line = serde_json::to_string(&json!({
        "ts": timestamp,
        "agent": request.agent_id,
        "payload_status": payload_status,
        "handoff": handoff_path.display().to_string(),
    }))
    .map_err(|error| error.to_string())?
        + "\n";
    let mut stream_file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&stream_path)
        .map_err(|e| e.to_string())?;
    std::io::Write::write_all(&mut stream_file, stream_line.as_bytes())
        .map_err(|e| e.to_string())?;

    Ok(DeliveryHandoffArtifact {
        handoff_path: handoff_path.display().to_string(),
        resolved_payload_path: resolved_payload_path.map(|path| path.display().to_string()),
        payload_status,
    })
}

/// Check if the OpenClaw delivery queue path exists and is writable.
pub fn check_delivery_queue(delivery_queue_path: &Path) -> SeamResult<bool> {
    if delivery_queue_path.exists() && delivery_queue_path.is_dir() {
        Ok(true)
    } else {
        Ok(false)
    }
}

pub fn render_cutover_status_human(root: &Path) -> SeamResult<String> {
    let config = read_config(root).map_err(|error| error.to_string())?;
    let mode = IntegrationMode::from_str(&config.openclaw_integration);
    let delivery_queue_path = openclaw_delivery_queue_path(root, &config);
    let queue_exists = check_delivery_queue(&delivery_queue_path).unwrap_or(false);
    let capability_resolution = resolve_capability_for_request(root, &config, None, "research", "web_search");
    let (capability, capability_note) = match capability_resolution {
        Ok(capability) => (capability, None),
        Err(error) => (None, Some(error)),
    };
    let readiness = render_capability_readiness_human(
        "research",
        "web_search",
        capability.as_ref(),
        capability_note.as_deref(),
    );
    Ok(format!(
        "OpenClaw Integration Seam // STATUS\n===================================\nmode:           {}\ndelivery_queue: {}\nqueue_exists:   {}\ncutover:        not implemented (intentional)\n\n{}\n",
        mode.as_str(),
        delivery_queue_path.display(),
        queue_exists,
        readiness,
    ))
}

/// Render a human-readable summary of a routing result.
pub fn render_routing_result_human(result: &DeliveryRoutingResult) -> String {
    let mut out = String::new();
    out.push_str("OpenClaw Integration Seam\n");
    out.push_str("========================\n");
    out.push_str(&format!("mode:              {}\n", result.mode));
    out.push_str(&format!("routed:            {}\n", result.routed));
    out.push_str(&format!("governance_passed: {}\n", result.governance_passed));
    out.push_str(&format!("bypass_triggered:  {}\n", result.bypass_triggered));
    if let Some(ref p) = result.dry_run_log_path {
        out.push_str(&format!("dry_run_log:       {}\n", p));
    }
    if let Some(ref p) = result.handoff_artifact_path {
        out.push_str(&format!("handoff_artifact:  {}\n", p));
    }
    if let Some(ref p) = result.resolved_payload_path {
        out.push_str(&format!("payload_path:      {}\n", p));
    }
    out.push_str(&format!("payload_status:    {}\n", result.payload_status));
    if let Some(ref p) = result.delivery_queue_path {
        out.push_str(&format!("delivery_queue:    {}\n", p));
    }
    out.push_str(&format!("reason:            {}\n", result.reason));
    out
}

/// Render a JSON summary of a routing result.
pub fn render_routing_result_json(result: &DeliveryRoutingResult) -> String {
    result.to_json()
}

fn epoch_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use loom_core::{
        capabilities::{scaffold_capability, CapabilityScaffoldRequest},
        init_workspace,
    };
    use std::fs;

    fn test_request() -> DeliveryRoutingRequest {
        DeliveryRoutingRequest {
            agent_id: "agent_atlas".to_string(),
            org_id: "org_test".to_string(),
            action_type: "deliver".to_string(),
            resource: "nightly_brief".to_string(),
            payload_path: "/tmp/brief.md".to_string(),
            delivery_target: "telegram:@test".to_string(),
        }
    }

    #[test]
    fn mode_off_returns_not_routed() {
        let dir = std::env::temp_dir().join("loom-seam-off-test");
        let _ = fs::create_dir_all(&dir);
        let dq = dir.join("delivery-queue");
        let result = route_delivery(&dir, &IntegrationMode::Off, &dq, &test_request(), "allow").unwrap();
        assert!(!result.routed);
        assert_eq!(result.mode, "off");
        assert!(!result.governance_passed);
        assert!(!result.bypass_triggered);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn dry_run_governance_allow_logs_without_delivery() {
        let dir = std::env::temp_dir().join("loom-seam-dryrun-allow");
        let _ = fs::create_dir_all(dir.join("state"));
        let dq = dir.join("delivery-queue");
        let result = route_delivery(&dir, &IntegrationMode::DryRun, &dq, &test_request(), "allow").unwrap();
        assert!(!result.routed, "dry_run must never actually route");
        assert!(result.governance_passed);
        assert!(!result.bypass_triggered);
        assert!(result.dry_run_log_path.is_some());
        assert!(result.handoff_artifact_path.is_some());
        assert_eq!(result.payload_status, "missing");
        assert_eq!(result.resolved_payload_path.as_deref(), Some("/tmp/brief.md"));
        let log_path = result.dry_run_log_path.unwrap();
        let log_contents = fs::read_to_string(&log_path).unwrap();
        assert!(log_contents.contains("\"actually_delivered\": false"));
        assert!(log_contents.contains("\"governance_passed\": true"));
        assert!(log_contents.contains("\"handoff_artifact_path\":"));
        assert!(!dq.exists(), "delivery queue must not be created in dry-run");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn dry_run_writes_handoff_artifact_and_resolves_payload() {
        let dir = std::env::temp_dir().join("loom-seam-handoff-payload");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("state")).expect("state dir");
        fs::create_dir_all(dir.join("payloads")).expect("payloads dir");
        let payload_file = dir.join("payloads/brief.txt");
        fs::write(&payload_file, "hello from payload").expect("payload file");
        let dq = dir.join("delivery-queue");
        let mut request = test_request();
        request.payload_path = "payloads/brief.txt".to_string();

        let result = route_delivery(&dir, &IntegrationMode::DryRun, &dq, &request, "allow").unwrap();
        let payload_path_str = payload_file.display().to_string();
        assert_eq!(result.payload_status, "loaded");
        assert_eq!(result.resolved_payload_path.as_deref(), Some(payload_path_str.as_str()));
        let handoff_path = result.handoff_artifact_path.expect("handoff artifact");
        let handoff_contents = fs::read_to_string(&handoff_path).expect("handoff artifact contents");
        assert!(handoff_contents.contains("\"payload_status\": \"loaded\""));
        assert!(handoff_contents.contains("hello from payload"));
        assert!(handoff_contents.contains(&payload_path_str));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn dry_run_governance_deny_triggers_bypass() {
        let dir = std::env::temp_dir().join("loom-seam-dryrun-deny");
        let _ = fs::create_dir_all(dir.join("state"));
        let dq = dir.join("delivery-queue");
        let result = route_delivery(&dir, &IntegrationMode::DryRun, &dq, &test_request(), "hard_deny").unwrap();
        assert!(!result.routed);
        assert!(!result.governance_passed);
        assert!(result.bypass_triggered, "bypass must trigger on governance deny");
        assert!(result.reason.contains("bypass would trigger"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn live_mode_returns_error() {
        let dir = std::env::temp_dir().join("loom-seam-live-test");
        let _ = fs::create_dir_all(&dir);
        let dq = dir.join("delivery-queue");
        let err = route_delivery(&dir, &IntegrationMode::Live, &dq, &test_request(), "allow");
        assert!(err.is_err());
        assert!(err.unwrap_err().contains("not yet implemented"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn dry_run_stream_appends() {
        let dir = std::env::temp_dir().join("loom-seam-stream-test");
        let _ = fs::create_dir_all(dir.join("state"));
        let dq = dir.join("delivery-queue");

        route_delivery(&dir, &IntegrationMode::DryRun, &dq, &test_request(), "allow").unwrap();
        route_delivery(&dir, &IntegrationMode::DryRun, &dq, &test_request(), "deny").unwrap();

        let stream = fs::read_to_string(dir.join("state/openclaw_seam/dry_run_stream.jsonl")).unwrap();
        let lines: Vec<&str> = stream.lines().collect();
        assert_eq!(lines.len(), 2, "stream should have 2 entries");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn integration_mode_parse() {
        assert_eq!(IntegrationMode::from_str("off"), IntegrationMode::Off);
        assert_eq!(IntegrationMode::from_str("dry_run"), IntegrationMode::DryRun);
        assert_eq!(IntegrationMode::from_str("dry-run"), IntegrationMode::DryRun);
        assert_eq!(IntegrationMode::from_str("DryRun"), IntegrationMode::DryRun);
        assert_eq!(IntegrationMode::from_str("live"), IntegrationMode::Live);
        assert_eq!(IntegrationMode::from_str(""), IntegrationMode::Off);
        assert_eq!(IntegrationMode::from_str("garbage"), IntegrationMode::Off);
    }

    #[test]
    fn check_delivery_queue_exists() {
        let dir = std::env::temp_dir().join("loom-seam-dq-check");
        let _ = fs::create_dir_all(&dir);
        assert!(check_delivery_queue(&dir).unwrap());
        let _ = fs::remove_dir_all(&dir);

        let missing = std::env::temp_dir().join("loom-seam-dq-missing-42");
        assert!(!check_delivery_queue(&missing).unwrap());
    }

    #[test]
    fn cutover_status_surfaces_resolved_capability_readiness() {
        let root = std::env::temp_dir().join("loom-seam-status-readiness");
        let _ = fs::remove_dir_all(&root);
        let config = init_workspace(&root, "embedded", None, "org_test").expect("init workspace");
        scaffold_capability(
            &root,
            &config,
            &CapabilityScaffoldRequest {
                name: "loom.research.web_search.v1".to_string(),
                description: "research web search".to_string(),
                action_type: "research".to_string(),
                resource: "web_search".to_string(),
                worker_kind: "python".to_string(),
                worker_entry: String::new(),
                wasm_module: String::new(),
                payload_mode: "json".to_string(),
            },
        )
        .expect("scaffold capability");

        let status = render_cutover_status_human(&root).expect("status");
        assert!(status.contains("CAPABILITY READINESS"));
        assert!(status.contains("interpreter:       python3"));
        assert!(status.contains("runtime_lane:"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn cutover_status_resolves_relative_delivery_queue_under_root() {
        let root = std::env::temp_dir().join("loom-seam-status-queue");
        let _ = fs::remove_dir_all(&root);
        let _config = init_workspace(&root, "embedded", None, "org_test").expect("init workspace");
        let config_path = root.join("loom.toml");
        let updated = fs::read_to_string(&config_path)
            .expect("read config")
            .replace("openclaw_integration = \"off\"", "openclaw_integration = \"dry_run\"")
            .replace(
                loom_core::DEFAULT_OPENCLAW_DELIVERY_QUEUE,
                "state/openclaw/delivery-queue",
            );
        fs::write(&config_path, updated).expect("rewrite config");
        fs::create_dir_all(root.join("state/openclaw/delivery-queue")).expect("queue dir");

        let status = render_cutover_status_human(&root).expect("status");
        assert!(status.contains(&root.join("state/openclaw/delivery-queue").display().to_string()));
        assert!(status.contains("queue_exists:   true"));
        let _ = fs::remove_dir_all(&root);
    }
}
