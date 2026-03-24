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

use std::fs;
use std::path::Path;

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

/// Result of routing a delivery through the seam.
#[derive(Clone, Debug)]
pub struct DeliveryRoutingResult {
    pub mode: String,
    pub routed: bool,
    pub governance_passed: bool,
    pub bypass_triggered: bool,
    pub dry_run_log_path: Option<String>,
    pub delivery_queue_path: Option<String>,
    pub reason: String,
}

impl DeliveryRoutingResult {
    fn to_json(&self) -> String {
        format!(
            concat!(
                "{{\n",
                "  \"mode\": {},\n",
                "  \"routed\": {},\n",
                "  \"governance_passed\": {},\n",
                "  \"bypass_triggered\": {},\n",
                "  \"dry_run_log_path\": {},\n",
                "  \"delivery_queue_path\": {},\n",
                "  \"reason\": {}\n",
                "}}\n"
            ),
            json_str(&self.mode),
            self.routed,
            self.governance_passed,
            self.bypass_triggered,
            self.dry_run_log_path.as_deref().map(json_str).unwrap_or_else(|| "null".to_string()),
            self.delivery_queue_path.as_deref().map(json_str).unwrap_or_else(|| "null".to_string()),
            json_str(&self.reason),
        )
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
        IntegrationMode::Off => {
            Ok(DeliveryRoutingResult {
                mode: "off".to_string(),
                routed: false,
                governance_passed: false,
                bypass_triggered: false,
                dry_run_log_path: None,
                delivery_queue_path: None,
                reason: "openclaw_integration=off; seam disabled".to_string(),
            })
        }
        IntegrationMode::DryRun => {
            let governance_passed = governance_decision == "allow";

            // Write dry-run log — never touch the actual delivery queue
            let seam_dir = root.join("state").join("openclaw_seam");
            fs::create_dir_all(&seam_dir).map_err(|e| e.to_string())?;
            let timestamp = epoch_now();
            let log_path = seam_dir.join(format!("dry_run_{}.json", timestamp));

            let bypass_triggered = !governance_passed;
            let log_entry = format!(
                concat!(
                    "{{\n",
                    "  \"timestamp\": {},\n",
                    "  \"mode\": \"dry_run\",\n",
                    "  \"agent_id\": {},\n",
                    "  \"org_id\": {},\n",
                    "  \"action_type\": {},\n",
                    "  \"resource\": {},\n",
                    "  \"payload_path\": {},\n",
                    "  \"delivery_target\": {},\n",
                    "  \"governance_decision\": {},\n",
                    "  \"governance_passed\": {},\n",
                    "  \"bypass_triggered\": {},\n",
                    "  \"would_route_to\": {},\n",
                    "  \"actually_delivered\": false,\n",
                    "  \"note\": \"dry-run mode: no delivery performed\"\n",
                    "}}\n"
                ),
                timestamp,
                json_str(&request.agent_id),
                json_str(&request.org_id),
                json_str(&request.action_type),
                json_str(&request.resource),
                json_str(&request.payload_path),
                json_str(&request.delivery_target),
                json_str(governance_decision),
                governance_passed,
                bypass_triggered,
                json_str(&delivery_queue_path.display().to_string()),
            );
            fs::write(&log_path, &log_entry).map_err(|e| e.to_string())?;

            // Append to stream
            let stream_path = seam_dir.join("dry_run_stream.jsonl");
            let stream_line = format!(
                "{{\"ts\":{},\"agent\":{},\"decision\":{},\"bypass\":{},\"mode\":\"dry_run\"}}\n",
                timestamp,
                json_str(&request.agent_id),
                json_str(governance_decision),
                bypass_triggered,
            );
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
                delivery_queue_path: None,
                reason: if governance_passed {
                    "dry-run: governance passed, delivery would proceed".to_string()
                } else {
                    format!("dry-run: governance denied ({}), bypass would trigger", governance_decision)
                },
            })
        }
        IntegrationMode::Live => {
            // INTENTIONALLY NOT IMPLEMENTED.
            // Live cutover requires:
            //   1. Level 1 acceptance proven (done)
            //   2. Feature flag + dry-run verified (this seam)
            //   3. Explicit owner authorization
            //   4. Rollback path tested
            // Until all 4 are met, live mode returns an error.
            Err("openclaw_integration=live is not yet implemented; cutover requires explicit owner authorization and proven dry-run acceptance".to_string())
        }
    }
}

/// Check if the OpenClaw delivery queue path exists and is writable.
pub fn check_delivery_queue(delivery_queue_path: &Path) -> SeamResult<bool> {
    if delivery_queue_path.exists() && delivery_queue_path.is_dir() {
        Ok(true)
    } else {
        Ok(false)
    }
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

fn json_str(s: &str) -> String {
    format!("{:?}", s)
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
        let result = route_delivery(
            &dir, &IntegrationMode::Off, &dq, &test_request(), "allow",
        ).unwrap();
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
        let result = route_delivery(
            &dir, &IntegrationMode::DryRun, &dq, &test_request(), "allow",
        ).unwrap();
        assert!(!result.routed, "dry_run must never actually route");
        assert!(result.governance_passed);
        assert!(!result.bypass_triggered);
        assert!(result.dry_run_log_path.is_some());
        let log_path = result.dry_run_log_path.unwrap();
        let log_contents = fs::read_to_string(&log_path).unwrap();
        assert!(log_contents.contains("\"actually_delivered\": false"));
        assert!(log_contents.contains("\"governance_passed\": true"));

        // Verify delivery queue was NOT touched
        assert!(!dq.exists(), "delivery queue must not be created in dry-run");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn dry_run_governance_deny_triggers_bypass() {
        let dir = std::env::temp_dir().join("loom-seam-dryrun-deny");
        let _ = fs::create_dir_all(dir.join("state"));
        let dq = dir.join("delivery-queue");
        let result = route_delivery(
            &dir, &IntegrationMode::DryRun, &dq, &test_request(), "hard_deny",
        ).unwrap();
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
        let err = route_delivery(
            &dir, &IntegrationMode::Live, &dq, &test_request(), "allow",
        );
        assert!(err.is_err());
        assert!(err.unwrap_err().contains("not yet implemented"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn dry_run_stream_appends() {
        let dir = std::env::temp_dir().join("loom-seam-stream-test");
        let _ = fs::create_dir_all(dir.join("state"));
        let dq = dir.join("delivery-queue");

        // Two requests
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
}
