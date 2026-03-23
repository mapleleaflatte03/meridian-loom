use loom_core::{
    build_action_envelope, envelope_input_hash, ensure_runtime_worker_scaffold,
    evaluate_reference_gates, kernel_path_for, preview_local_sanction_controls, read_config,
    resolve_agent_identity, runtime_worker_entry, ActionEnvelope, AgentIdentityResolution, Config,
    ReferenceGateCheck,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub type ShadowResult<T> = Result<T, String>;

#[derive(Clone, Debug, PartialEq)]
pub struct PreflightCapture {
    pub event_log: PathBuf,
    pub audit_preview_log: PathBuf,
    pub reference_report: PathBuf,
    pub reference_event_log: PathBuf,
    pub latest_report: PathBuf,
    pub input_hash: String,
    pub hooks: Vec<String>,
    pub estimated_cost_usd: f64,
    pub identity_restrictions: Vec<String>,
    pub reference_restrictions: Vec<String>,
    pub sanction_decision: String,
    pub sanction_gate_decision: String,
    pub budget_limit_usd: Option<f64>,
    pub budget_gate_decision: String,
    pub approval_decision: String,
    pub approval_gate_decision: String,
    pub audit_emission_decision: String,
    pub overall_decision: String,
    pub reference_stage: String,
    pub reference_reason: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ComparisonSummary {
    pub primary_log: PathBuf,
    pub shadow_log: PathBuf,
    pub primary_events: usize,
    pub shadow_events: usize,
    pub pairs_compared: usize,
    pub matches: usize,
    pub divergences: usize,
    pub divergence_rate: f64,
    pub hook_results: Vec<HookComparison>,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DecisionCapture {
    pub decision_path: PathBuf,
    pub input_hash: String,
    pub agent_id: String,
    pub org_id: String,
    pub action_type: String,
    pub resource: String,
    pub estimated_cost_usd: f64,
    pub identity_restrictions: Vec<String>,
    pub reference_restrictions: Vec<String>,
    pub local_sanction_allowed: bool,
    pub local_sanction_decision: String,
    pub local_sanction_reason: String,
    pub sanction_gate_decision: String,
    pub approval_gate_decision: String,
    pub budget_limit_usd: Option<f64>,
    pub budget_gate_decision: String,
    pub overall_decision: String,
    pub effective_source: String,
    pub effective_stage: String,
    pub effective_reason: String,
    pub reference_stage: String,
    pub reference_reason: String,
    pub source: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeExecutionCapture {
    pub execution_path: PathBuf,
    pub worker_request_path: PathBuf,
    pub worker_result_path: PathBuf,
    pub worker_log_path: PathBuf,
    pub audit_log_path: PathBuf,
    pub parity_stream_path: PathBuf,
    pub parity_report_path: PathBuf,
    pub openclaw_live_probe_path: Option<PathBuf>,
    pub openclaw_live_probe_stream_path: Option<PathBuf>,
    pub decision_path: PathBuf,
    pub input_hash: String,
    pub agent_id: String,
    pub org_id: String,
    pub action_type: String,
    pub resource: String,
    pub estimated_cost_usd: f64,
    pub runtime_outcome: String,
    pub worker_status: String,
    pub worker_kind: String,
    pub worker_note: String,
    pub overall_decision: String,
    pub effective_source: String,
    pub effective_stage: String,
    pub reference_decision: String,
    pub reference_stage: String,
    pub audit_emission_status: String,
    pub openclaw_live_probe_status: String,
    pub openclaw_live_probe_note: String,
    pub parity_status: String,
    pub parity_reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnqueuedAction {
    pub queue_path: PathBuf,
    pub job_path: PathBuf,
    pub input_hash: String,
    pub agent_id: String,
    pub org_id: String,
    pub action_type: String,
    pub resource: String,
    pub estimated_cost_usd: String,
    pub kernel_path: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JobSnapshot {
    pub root: PathBuf,
    pub job_id: String,
    pub job_path: PathBuf,
    pub status: String,
    pub stage: String,
    pub queue_bucket: String,
    pub queued_at: String,
    pub updated_at: String,
    pub agent_id: String,
    pub org_id: String,
    pub action_type: String,
    pub resource: String,
    pub estimated_cost_usd: String,
    pub runtime_outcome: String,
    pub worker_status: String,
    pub queue_path: Option<PathBuf>,
    pub decision_path: Option<PathBuf>,
    pub execution_path: Option<PathBuf>,
    pub audit_log_path: Option<PathBuf>,
    pub parity_report_path: Option<PathBuf>,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SupervisorRunSummary {
    pub root: PathBuf,
    pub queue_dir: PathBuf,
    pub processed: usize,
    pub allowed: usize,
    pub denied: usize,
    pub failed: usize,
    pub last_input_hash: String,
    pub last_execution_path: PathBuf,
    pub audit_log_path: PathBuf,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SupervisorWatchSummary {
    pub root: PathBuf,
    pub supervisor_dir: PathBuf,
    pub iterations: usize,
    pub poll_seconds: u64,
    pub processed: usize,
    pub allowed: usize,
    pub denied: usize,
    pub failed: usize,
    pub heartbeat_log_path: PathBuf,
    pub status_path: PathBuf,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SupervisorStatusSnapshot {
    pub root: PathBuf,
    pub supervisor_dir: PathBuf,
    pub status_path: PathBuf,
    pub heartbeat_log_path: PathBuf,
    pub available: bool,
    pub updated_at: String,
    pub iterations: usize,
    pub poll_seconds: u64,
    pub processed: usize,
    pub allowed: usize,
    pub denied: usize,
    pub failed: usize,
    pub pending_jobs: usize,
    pub processed_jobs: usize,
    pub failed_jobs: usize,
    pub heartbeat_entries: usize,
    pub last_heartbeat_timestamp: String,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SupervisorDaemonSnapshot {
    pub root: PathBuf,
    pub supervisor_dir: PathBuf,
    pub runtime_state_path: PathBuf,
    pub stop_request_path: PathBuf,
    pub stdout_log_path: PathBuf,
    pub available: bool,
    pub session_id: String,
    pub pid: u32,
    pub running: bool,
    pub status: String,
    pub updated_at: String,
    pub booted_at: String,
    pub stopped_at: String,
    pub poll_seconds: u64,
    pub max_jobs: usize,
    pub max_iterations: usize,
    pub iterations_completed: usize,
    pub processed: usize,
    pub allowed: usize,
    pub denied: usize,
    pub failed: usize,
    pub pending_jobs: usize,
    pub processed_jobs: usize,
    pub failed_jobs: usize,
    pub heartbeat_entries: usize,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq)]
struct WorkerExecutionCapture {
    worker_request_path: PathBuf,
    worker_result_path: PathBuf,
    worker_log_path: PathBuf,
    worker_status: String,
    worker_kind: String,
    worker_note: String,
    runtime_outcome: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HookComparison {
    pub pair_index: usize,
    pub hook_name: String,
    pub input_hash: String,
    pub primary_decision: String,
    pub shadow_decision: String,
    pub matched: bool,
    pub primary_agent_id: String,
    pub shadow_agent_id: String,
    pub primary_org_id: String,
    pub shadow_org_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ShadowEvent {
    hook_name: String,
    input_hash: String,
    decision: String,
    agent_id: String,
    org_id: String,
}

pub fn capture_preflight(
    root: &Path,
    kernel_path: &Path,
    identity: &AgentIdentityResolution,
    envelope: &ActionEnvelope,
    reference: &ReferenceGateCheck,
) -> ShadowResult<PreflightCapture> {
    let shadow_dir = ensure_shadow_dir(root)?;
    let audit_dir = ensure_audit_dir(root)?;
    let event_log = shadow_dir.join("events.jsonl");
    let audit_preview_log = audit_dir.join("preview.jsonl");
    let reference_report = shadow_dir.join("reference.json");
    let reference_event_log = shadow_dir.join("reference_events.jsonl");
    let latest_report = shadow_dir.join("latest.json");
    let input_hash = envelope_input_hash(envelope);
    let sanction_decision = if identity.sanction_decision.is_empty() {
        "unknown"
    } else {
        identity.sanction_decision.as_str()
    };
    let effective_restrictions = if !reference.restrictions.is_empty() {
        reference.restrictions.clone()
    } else {
        identity.restrictions.clone()
    };
    let budget_limit_usd = identity.max_per_run_usd;
    let approval_decision = if identity.approval_required {
        "requires_approval"
    } else {
        "not_required"
    };
    let overall_decision = if reference.allowed { "allow" } else { "deny" };

    append_line(
        &event_log,
        &format!(
            "{{\"timestamp\":{},\"source\":\"loom_shadow_preflight\",\"hook_name\":\"agent_identity\",\"decision\":\"resolved\",\"agent_id\":{},\"org_id\":{},\"runtime_id\":{},\"input_hash\":{},\"note\":\"experimental preflight capture\"}}\n",
            json_string(&timestamp_now()),
            json_string(&identity.agent_id),
            json_string(&identity.org_id),
            json_string(&identity.runtime_id),
            json_string(&input_hash),
        ),
    )?;
    append_line(
        &event_log,
        &format!(
            "{{\"timestamp\":{},\"source\":\"loom_shadow_preflight\",\"hook_name\":\"action_envelope\",\"decision\":\"constructed\",\"agent_id\":{},\"org_id\":{},\"runtime_id\":{},\"action_type\":{},\"resource\":{},\"estimated_cost_usd\":{:.6},\"input_hash\":{},\"note\":\"experimental preflight capture\"}}\n",
            json_string(&timestamp_now()),
            json_string(&envelope.agent_id),
            json_string(&envelope.org_id),
            json_string(&envelope.runtime_id),
            json_string(&envelope.action_type),
            json_string(&envelope.resource),
            envelope.estimated_cost_usd,
            json_string(&input_hash),
        ),
    )?;
    append_line(
        &event_log,
        &format!(
            "{{\"timestamp\":{},\"source\":\"loom_shadow_preflight\",\"hook_name\":\"sanction_controls\",\"decision\":{},\"agent_id\":{},\"org_id\":{},\"runtime_id\":{},\"restrictions\":{},\"input_hash\":{},\"note\":\"experimental restriction snapshot\"}}\n",
            json_string(&timestamp_now()),
            json_string(&reference.sanction_gate_decision),
            json_string(&identity.agent_id),
            json_string(&identity.org_id),
            json_string(&identity.runtime_id),
            render_json_string_array(&effective_restrictions),
            json_string(&input_hash),
        ),
    )?;
    append_line(
        &event_log,
        &format!(
            "{{\"timestamp\":{},\"source\":\"loom_shadow_preflight\",\"hook_name\":\"cost_attribution\",\"decision\":\"estimated\",\"agent_id\":{},\"org_id\":{},\"runtime_id\":{},\"estimated_cost_usd\":{:.6},\"input_hash\":{},\"note\":\"experimental preflight estimate\"}}\n",
            json_string(&timestamp_now()),
            json_string(&envelope.agent_id),
            json_string(&envelope.org_id),
            json_string(&envelope.runtime_id),
            envelope.estimated_cost_usd,
            json_string(&input_hash),
        ),
    )?;
    append_line(
        &event_log,
        &format!(
            "{{\"timestamp\":{},\"source\":\"loom_shadow_preflight\",\"hook_name\":\"approval_hook\",\"decision\":{},\"agent_id\":{},\"org_id\":{},\"runtime_id\":{},\"input_hash\":{},\"note\":\"experimental preflight policy read\"}}\n",
            json_string(&timestamp_now()),
            json_string(&reference.approval_gate_decision),
            json_string(&identity.agent_id),
            json_string(&identity.org_id),
            json_string(&identity.runtime_id),
            json_string(&input_hash),
        ),
    )?;
    append_line(
        &event_log,
        &format!(
            "{{\"timestamp\":{},\"source\":\"loom_shadow_preflight\",\"hook_name\":\"budget_gate\",\"decision\":{},\"agent_id\":{},\"org_id\":{},\"runtime_id\":{},\"estimated_cost_usd\":{:.6},\"budget_limit_usd\":{},\"input_hash\":{},\"note\":\"experimental preflight budget check\"}}\n",
            json_string(&timestamp_now()),
            json_string(&reference.budget_gate_decision),
            json_string(&identity.agent_id),
            json_string(&identity.org_id),
            json_string(&identity.runtime_id),
            envelope.estimated_cost_usd,
            budget_limit_usd
                .map(|value| format!("{:.6}", value))
                .unwrap_or_else(|| "null".to_string()),
            json_string(&input_hash),
        ),
    )?;
    let audit_emission_decision =
        emit_kernel_audit_preview(kernel_path, &audit_preview_log, envelope, &input_hash)?;
    fs::write(
        &reference_report,
        format!(
            "{{\n  \"source\": {},\n  \"allowed\": {},\n  \"stage\": {},\n  \"reason\": {},\n  \"restrictions\": {},\n  \"sanction_gate_decision\": {},\n  \"approval_gate_decision\": {},\n  \"budget_gate_decision\": {}\n}}\n",
            json_string(&reference.source),
            if reference.allowed { "true" } else { "false" },
            json_string(&reference.stage),
            json_string(&reference.reason),
            render_json_string_array(&reference.restrictions),
            json_string(&reference.sanction_gate_decision),
            json_string(&reference.approval_gate_decision),
            json_string(&reference.budget_gate_decision),
        ),
    )
    .map_err(io_err)?;
    fs::write(
        &reference_event_log,
        format!(
            "{{\"timestamp\":{},\"source\":\"kernel_reference_adapter\",\"hook_name\":\"agent_identity\",\"decision\":\"resolved\",\"agent_id\":{},\"org_id\":{},\"input_hash\":{}}}\n\
{{\"timestamp\":{},\"source\":\"kernel_reference_adapter\",\"hook_name\":\"action_envelope\",\"decision\":\"constructed\",\"agent_id\":{},\"org_id\":{},\"input_hash\":{}}}\n\
{{\"timestamp\":{},\"source\":\"kernel_reference_adapter\",\"hook_name\":\"sanction_controls\",\"decision\":{},\"agent_id\":{},\"org_id\":{},\"input_hash\":{}}}\n\
{{\"timestamp\":{},\"source\":\"kernel_reference_adapter\",\"hook_name\":\"cost_attribution\",\"decision\":\"estimated\",\"agent_id\":{},\"org_id\":{},\"input_hash\":{}}}\n\
{{\"timestamp\":{},\"source\":\"kernel_reference_adapter\",\"hook_name\":\"approval_hook\",\"decision\":{},\"agent_id\":{},\"org_id\":{},\"input_hash\":{}}}\n\
{{\"timestamp\":{},\"source\":\"kernel_reference_adapter\",\"hook_name\":\"budget_gate\",\"decision\":{},\"agent_id\":{},\"org_id\":{},\"input_hash\":{}}}\n\
{{\"timestamp\":{},\"source\":\"kernel_reference_adapter\",\"hook_name\":\"audit_emission\",\"decision\":\"not_exercised\",\"agent_id\":{},\"org_id\":{},\"input_hash\":{}}}\n",
            json_string(&timestamp_now()),
            json_string(&identity.agent_id),
            json_string(&identity.org_id),
            json_string(&input_hash),
            json_string(&timestamp_now()),
            json_string(&identity.agent_id),
            json_string(&identity.org_id),
            json_string(&input_hash),
            json_string(&timestamp_now()),
            json_string(&reference.sanction_gate_decision),
            json_string(&identity.agent_id),
            json_string(&identity.org_id),
            json_string(&input_hash),
            json_string(&timestamp_now()),
            json_string(&identity.agent_id),
            json_string(&identity.org_id),
            json_string(&input_hash),
            json_string(&timestamp_now()),
            json_string(&reference.approval_gate_decision),
            json_string(&identity.agent_id),
            json_string(&identity.org_id),
            json_string(&input_hash),
            json_string(&timestamp_now()),
            json_string(&reference.budget_gate_decision),
            json_string(&identity.agent_id),
            json_string(&identity.org_id),
            json_string(&input_hash),
            json_string(&timestamp_now()),
            json_string(&identity.agent_id),
            json_string(&identity.org_id),
            json_string(&input_hash),
        ),
    )
    .map_err(io_err)?;
    append_line(
        &event_log,
        &format!(
            "{{\"timestamp\":{},\"source\":\"loom_shadow_preflight\",\"hook_name\":\"audit_emission\",\"decision\":{},\"agent_id\":{},\"org_id\":{},\"runtime_id\":{},\"input_hash\":{},\"preview_log\":{},\"note\":\"experimental audit preview written via kernel serializer\"}}\n",
            json_string(&timestamp_now()),
            json_string(&audit_emission_decision),
            json_string(&envelope.agent_id),
            json_string(&envelope.org_id),
            json_string(&envelope.runtime_id),
            json_string(&input_hash),
            json_string(&audit_preview_log.display().to_string()),
        ),
    )?;

    fs::write(
        &latest_report,
        format!(
            "{{\n  \"status\": \"preflight_captured\",\n  \"events_compared\": 0,\n  \"divergences\": 0,\n  \"captured_hooks\": [\"agent_identity\", \"action_envelope\", \"cost_attribution\", \"approval_hook\", \"audit_emission\", \"sanction_controls\", \"budget_gate\"],\n  \"input_hash\": {},\n  \"estimated_cost_usd\": {:.6},\n  \"restrictions\": {},\n  \"sanction_decision\": {},\n  \"sanction_gate_decision\": {},\n  \"budget_limit_usd\": {},\n  \"budget_gate_decision\": {},\n  \"approval_decision\": {},\n  \"approval_gate_decision\": {},\n  \"audit_emission_decision\": {},\n  \"overall_decision\": {},\n  \"reference_stage\": {},\n  \"reference_reason\": {},\n  \"event_log\": {},\n  \"audit_preview_log\": {},\n  \"reference_report\": {},\n  \"reference_event_log\": {},\n  \"note\": \"experimental preflight captured with read-only kernel reference gates; no governed runtime yet\"\n}}\n",
            json_string(&input_hash),
            envelope.estimated_cost_usd,
            render_json_string_array(&effective_restrictions),
            json_string(sanction_decision),
            json_string(&reference.sanction_gate_decision),
            budget_limit_usd
                .map(|value| format!("{:.6}", value))
                .unwrap_or_else(|| "null".to_string()),
            json_string(&reference.budget_gate_decision),
            json_string(approval_decision),
            json_string(&reference.approval_gate_decision),
            json_string(&audit_emission_decision),
            json_string(overall_decision),
            json_string(&reference.stage),
            json_string(&reference.reason),
            json_string(&event_log.display().to_string()),
            json_string(&audit_preview_log.display().to_string()),
            json_string(&reference_report.display().to_string()),
            json_string(&reference_event_log.display().to_string()),
        ),
    )
    .map_err(io_err)?;

    Ok(PreflightCapture {
        event_log,
        audit_preview_log,
        reference_report,
        reference_event_log,
        latest_report,
        input_hash,
        hooks: vec![
            "agent_identity".to_string(),
            "action_envelope".to_string(),
            "cost_attribution".to_string(),
            "approval_hook".to_string(),
            "audit_emission".to_string(),
            "sanction_controls".to_string(),
            "budget_gate".to_string(),
        ],
        estimated_cost_usd: envelope.estimated_cost_usd,
        identity_restrictions: identity.restrictions.clone(),
        reference_restrictions: effective_restrictions,
        sanction_decision: sanction_decision.to_string(),
        sanction_gate_decision: reference.sanction_gate_decision.clone(),
        budget_limit_usd,
        budget_gate_decision: reference.budget_gate_decision.clone(),
        approval_decision: approval_decision.to_string(),
        approval_gate_decision: reference.approval_gate_decision.clone(),
        audit_emission_decision: audit_emission_decision.to_string(),
        overall_decision: overall_decision.to_string(),
        reference_stage: reference.stage.clone(),
        reference_reason: reference.reason.clone(),
    })
}

pub fn compare_logs(
    root: Option<&Path>,
    primary: &Path,
    shadow: &Path,
) -> ShadowResult<ComparisonSummary> {
    let primary_events = load_events(primary)?;
    let shadow_events = load_events(shadow)?;
    let pairs_compared = primary_events.len().min(shadow_events.len());
    let mut matches = 0usize;
    let mut divergences = 0usize;
    let mut hook_results = Vec::new();

    for idx in 0..pairs_compared {
        let left = &primary_events[idx];
        let right = &shadow_events[idx];
        let matched = left.hook_name == right.hook_name
            && left.input_hash == right.input_hash
            && left.decision == right.decision
            && left.agent_id == right.agent_id
            && left.org_id == right.org_id;
        if matched {
            matches += 1;
        } else {
            divergences += 1;
        }
        hook_results.push(HookComparison {
            pair_index: idx,
            hook_name: if left.hook_name == right.hook_name {
                left.hook_name.clone()
            } else {
                format!("{} -> {}", left.hook_name, right.hook_name)
            },
            input_hash: if left.input_hash == right.input_hash {
                left.input_hash.clone()
            } else {
                format!("{} -> {}", left.input_hash, right.input_hash)
            },
            primary_decision: left.decision.clone(),
            shadow_decision: right.decision.clone(),
            matched,
            primary_agent_id: left.agent_id.clone(),
            shadow_agent_id: right.agent_id.clone(),
            primary_org_id: left.org_id.clone(),
            shadow_org_id: right.org_id.clone(),
        });
    }

    divergences += primary_events.len().max(shadow_events.len()) - pairs_compared;
    let divergence_rate = if pairs_compared == 0 {
        0.0
    } else {
        divergences as f64 / pairs_compared as f64
    };

    let summary = ComparisonSummary {
        primary_log: primary.to_path_buf(),
        shadow_log: shadow.to_path_buf(),
        primary_events: primary_events.len(),
        shadow_events: shadow_events.len(),
        pairs_compared,
        matches,
        divergences,
        divergence_rate,
        hook_results,
        note: "offline event-log diff only; use `loom action execute` plus `loom parity report` for runtime-side rehearsal and optional live OpenClaw probe".to_string(),
    };

    if let Some(root) = root {
        let latest_report = ensure_shadow_dir(root)?.join("latest.json");
        fs::write(&latest_report, render_compare_json(&summary)).map_err(io_err)?;
    }

    Ok(summary)
}

pub fn capture_decision(
    root: &Path,
    identity: &AgentIdentityResolution,
    envelope: &ActionEnvelope,
    reference: &ReferenceGateCheck,
) -> ShadowResult<DecisionCapture> {
    let decision_path = ensure_shadow_dir(root)?.join("decision.json");
    let local_preview = preview_local_sanction_controls(identity);
    let (overall_decision, effective_source, effective_stage, effective_reason) =
        if !local_preview.allowed {
            (
                "deny".to_string(),
                "local_sanction_preview".to_string(),
                "sanction_controls_preview".to_string(),
                local_preview.reason.clone(),
            )
        } else if reference.allowed {
            (
                "allow".to_string(),
                "reference_gate".to_string(),
                reference.stage.clone(),
                reference.reason.clone(),
            )
        } else {
            (
                "deny".to_string(),
                "reference_gate".to_string(),
                reference.stage.clone(),
                reference.reason.clone(),
            )
        };
    let capture = DecisionCapture {
        decision_path: decision_path.clone(),
        input_hash: envelope_input_hash(envelope),
        agent_id: identity.agent_id.clone(),
        org_id: identity.org_id.clone(),
        action_type: envelope.action_type.clone(),
        resource: envelope.resource.clone(),
        estimated_cost_usd: envelope.estimated_cost_usd,
        identity_restrictions: identity.restrictions.clone(),
        reference_restrictions: reference.restrictions.clone(),
        local_sanction_allowed: local_preview.allowed,
        local_sanction_decision: local_preview.decision,
        local_sanction_reason: local_preview.reason,
        sanction_gate_decision: reference.sanction_gate_decision.clone(),
        approval_gate_decision: reference.approval_gate_decision.clone(),
        budget_limit_usd: identity.max_per_run_usd,
        budget_gate_decision: reference.budget_gate_decision.clone(),
        overall_decision,
        effective_source,
        effective_stage,
        effective_reason,
        reference_stage: reference.stage.clone(),
        reference_reason: reference.reason.clone(),
        source: "experimental_preflight_gate".to_string(),
    };
    fs::write(&decision_path, render_decision_json(&capture)).map_err(io_err)?;
    Ok(capture)
}

pub fn capture_runtime_execution(
    root: &Path,
    kernel_path: &Path,
    envelope: &ActionEnvelope,
    reference: &ReferenceGateCheck,
    decision: &DecisionCapture,
) -> ShadowResult<RuntimeExecutionCapture> {
    let runtime_dir = ensure_runtime_dir(root)?;
    let parity_dir = ensure_parity_dir(root)?;
    let audit_log_path = runtime_audit_log_path(root, Some(kernel_path.to_string_lossy().as_ref()));
    let execution_path = runtime_dir.join("last_execution.json");
    let parity_stream_path = parity_dir.join("stream.jsonl");
    let parity_report_path = parity_dir.join("latest.json");
    let worker_capture = run_worker_supervisor(root, envelope, decision)?;
    let (
        openclaw_live_probe_path,
        openclaw_live_probe_stream_path,
        openclaw_live_probe_status,
        openclaw_live_probe_note,
    ) = capture_openclaw_live_probe(root, &decision.input_hash)?;
    let runtime_outcome = worker_capture.runtime_outcome.clone();
    let audit_emission_status = emit_runtime_audit(
        kernel_path,
        &audit_log_path,
        envelope,
        decision,
        &runtime_outcome,
        &worker_capture,
    )?;
    let reference_decision = if reference.allowed { "allow" } else { "deny" }.to_string();
    let parity_status = if reference_decision == decision.overall_decision {
        "match".to_string()
    } else {
        "divergence".to_string()
    };
    let parity_reason = if parity_status == "match" {
        format!(
            "reference and loom agreed on {} at stage {}",
            decision.overall_decision, decision.reference_stage
        )
    } else {
        format!(
            "reference returned {} at stage {} while loom enforced {} via {}",
            reference_decision, decision.reference_stage, decision.overall_decision, decision.effective_stage
        )
    };

    let stream_timestamp = timestamp_now();
    append_line(
        &parity_stream_path,
        &format!(
            "{{\"timestamp\":{},\"source\":\"openclaw_reference_stream\",\"phase\":\"reference_gate\",\"hook_name\":{},\"decision\":{},\"stage\":{},\"agent_id\":{},\"org_id\":{},\"input_hash\":{},\"reason\":{}}}\n",
            json_string(&stream_timestamp),
            json_string(&decision.reference_stage),
            json_string(&reference_decision),
            json_string(&decision.reference_stage),
            json_string(&decision.agent_id),
            json_string(&decision.org_id),
            json_string(&decision.input_hash),
            json_string(&decision.reference_reason),
        ),
    )?;
    append_line(
        &parity_stream_path,
        &format!(
            "{{\"timestamp\":{},\"source\":\"loom_runtime_stream\",\"phase\":\"worker_supervisor\",\"hook_name\":{},\"decision\":{},\"stage\":{},\"agent_id\":{},\"org_id\":{},\"input_hash\":{},\"reason\":{},\"worker_status\":{},\"worker_kind\":{},\"worker_result_path\":{}}}\n",
            json_string(&stream_timestamp),
            json_string(&decision.effective_stage),
            json_string(&decision.overall_decision),
            json_string(&decision.effective_stage),
            json_string(&decision.agent_id),
            json_string(&decision.org_id),
            json_string(&decision.input_hash),
            json_string(&decision.effective_reason),
            json_string(&worker_capture.worker_status),
            json_string(&worker_capture.worker_kind),
            json_string(&worker_capture.worker_result_path.display().to_string()),
        ),
    )?;
    append_line(
        &parity_stream_path,
        &format!(
            "{{\"timestamp\":{},\"source\":\"loom_runtime_stream\",\"phase\":\"audit_emission\",\"hook_name\":\"audit_emission\",\"decision\":{},\"stage\":\"runtime_audit\",\"agent_id\":{},\"org_id\":{},\"input_hash\":{},\"reason\":{},\"audit_log\":{}}}\n",
            json_string(&stream_timestamp),
            json_string(&audit_emission_status),
            json_string(&decision.agent_id),
            json_string(&decision.org_id),
            json_string(&decision.input_hash),
            json_string(&runtime_outcome),
            json_string(&audit_log_path.display().to_string()),
        ),
    )?;
    append_line(
        &parity_stream_path,
        &format!(
            "{{\"timestamp\":{},\"source\":\"openclaw_live_probe\",\"phase\":\"live_runtime_probe\",\"hook_name\":\"runtime_health\",\"decision\":{},\"stage\":\"live_single_host_openclaw\",\"agent_id\":{},\"org_id\":{},\"input_hash\":{},\"reason\":{},\"probe_path\":{},\"probe_stream_path\":{}}}\n",
            json_string(&stream_timestamp),
            json_string(&openclaw_live_probe_status),
            json_string(&decision.agent_id),
            json_string(&decision.org_id),
            json_string(&decision.input_hash),
            json_string(&openclaw_live_probe_note),
            json_string(
                &openclaw_live_probe_path
                    .as_ref()
                    .map(|path| path.display().to_string())
                    .unwrap_or_default()
            ),
            json_string(
                &openclaw_live_probe_stream_path
                    .as_ref()
                    .map(|path| path.display().to_string())
                    .unwrap_or_default()
            ),
        ),
    )?;

    let capture = RuntimeExecutionCapture {
        execution_path: execution_path.clone(),
        worker_request_path: worker_capture.worker_request_path.clone(),
        worker_result_path: worker_capture.worker_result_path.clone(),
        worker_log_path: worker_capture.worker_log_path.clone(),
        audit_log_path: audit_log_path.clone(),
        parity_stream_path: parity_stream_path.clone(),
        parity_report_path: parity_report_path.clone(),
        openclaw_live_probe_path,
        openclaw_live_probe_stream_path,
        decision_path: decision.decision_path.clone(),
        input_hash: decision.input_hash.clone(),
        agent_id: decision.agent_id.clone(),
        org_id: decision.org_id.clone(),
        action_type: decision.action_type.clone(),
        resource: decision.resource.clone(),
        estimated_cost_usd: decision.estimated_cost_usd,
        runtime_outcome,
        worker_status: worker_capture.worker_status,
        worker_kind: worker_capture.worker_kind,
        worker_note: worker_capture.worker_note,
        overall_decision: decision.overall_decision.clone(),
        effective_source: decision.effective_source.clone(),
        effective_stage: decision.effective_stage.clone(),
        reference_decision,
        reference_stage: decision.reference_stage.clone(),
        audit_emission_status,
        openclaw_live_probe_status,
        openclaw_live_probe_note,
        parity_status,
        parity_reason,
    };
    fs::write(&execution_path, render_runtime_execution_json(&capture)).map_err(io_err)?;
    fs::write(&parity_report_path, render_parity_report_json(&capture)).map_err(io_err)?;
    let existing_job = read_job_snapshot(root, &decision.input_hash).ok();
    let queue_path = existing_job
        .as_ref()
        .and_then(|snapshot| snapshot.queue_path.clone());
    let queue_bucket = existing_job
        .as_ref()
        .map(|snapshot| snapshot.queue_bucket.clone())
        .unwrap_or_else(|| "(none)".to_string());
    let queued_at = existing_job
        .as_ref()
        .map(|snapshot| snapshot.queued_at.clone())
        .unwrap_or_else(timestamp_now);
    let status = if capture.overall_decision == "allow"
        && capture.worker_status == "completed"
        && capture.runtime_outcome == "worker_executed"
    {
        "completed".to_string()
    } else if capture.overall_decision == "deny" {
        "denied".to_string()
    } else if capture.worker_status == "failed" {
        "failed".to_string()
    } else {
        "runtime_rehearsed".to_string()
    };
    let stage = if queue_bucket == "processed" || queue_bucket == "failed" {
        "local_queue_supervisor".to_string()
    } else {
        "runtime_execute".to_string()
    };
    write_job_snapshot(
        root,
        JobSnapshot {
            root: root.to_path_buf(),
            job_id: decision.input_hash.clone(),
            job_path: job_snapshot_path(root, &decision.input_hash),
            status,
            stage,
            queue_bucket,
            queued_at,
            updated_at: timestamp_now(),
            agent_id: decision.agent_id.clone(),
            org_id: decision.org_id.clone(),
            action_type: decision.action_type.clone(),
            resource: decision.resource.clone(),
            estimated_cost_usd: format!("{:.6}", decision.estimated_cost_usd),
            runtime_outcome: capture.runtime_outcome.clone(),
            worker_status: capture.worker_status.clone(),
            queue_path,
            decision_path: Some(capture.decision_path.clone()),
            execution_path: Some(capture.execution_path.clone()),
            audit_log_path: Some(capture.audit_log_path.clone()),
            parity_report_path: Some(capture.parity_report_path.clone()),
            note: if capture.worker_note.is_empty() {
                format!("runtime execution {} via {}", capture.runtime_outcome, capture.effective_stage)
            } else {
                capture.worker_note.clone()
            },
        },
    )?;
    Ok(capture)
}

pub fn enqueue_action(
    root: &Path,
    kernel_path: &Path,
    envelope: &ActionEnvelope,
) -> ShadowResult<EnqueuedAction> {
    let pending_dir = ensure_runtime_dir(root)?.join("queue").join("pending");
    fs::create_dir_all(&pending_dir).map_err(io_err)?;
    let input_hash = envelope_input_hash(envelope);
    let queue_path = pending_dir.join(format!(
        "{}-{}-{}.json",
        timestamp_now(),
        sanitize_filename(&envelope.agent_id),
        &input_hash[..8]
    ));
    fs::write(
        &queue_path,
        format!(
            "{{\n  \"status\": \"queued\",\n  \"queued_at\": {},\n  \"input_hash\": {},\n  \"agent_id\": {},\n  \"org_id\": {},\n  \"action_type\": {},\n  \"resource\": {},\n  \"estimated_cost_usd\": {:.6},\n  \"run_id\": {},\n  \"session_id\": {},\n  \"kernel_path\": {}\n}}\n",
            json_string(&timestamp_now()),
            json_string(&input_hash),
            json_string(&envelope.agent_id),
            json_string(&envelope.org_id),
            json_string(&envelope.action_type),
            json_string(&envelope.resource),
            envelope.estimated_cost_usd,
            json_string(&envelope.run_id),
            json_string(&envelope.session_id),
            json_string(&kernel_path.display().to_string()),
        ),
    )
    .map_err(io_err)?;
    let job_path = job_snapshot_path(root, &input_hash);
    write_job_snapshot(
        root,
        JobSnapshot {
            root: root.to_path_buf(),
            job_id: input_hash.clone(),
            job_path: job_path.clone(),
            status: "queued".to_string(),
            stage: "queue_pending".to_string(),
            queue_bucket: "pending".to_string(),
            queued_at: timestamp_now(),
            updated_at: timestamp_now(),
            agent_id: envelope.agent_id.clone(),
            org_id: envelope.org_id.clone(),
            action_type: envelope.action_type.clone(),
            resource: envelope.resource.clone(),
            estimated_cost_usd: format!("{:.6}", envelope.estimated_cost_usd),
            runtime_outcome: "not_started".to_string(),
            worker_status: "queued".to_string(),
            queue_path: Some(queue_path.clone()),
            decision_path: None,
            execution_path: None,
            audit_log_path: None,
            parity_report_path: None,
            note: "queued for local supervisor rehearsal".to_string(),
        },
    )?;
    Ok(EnqueuedAction {
        queue_path,
        job_path,
        input_hash,
        agent_id: envelope.agent_id.clone(),
        org_id: envelope.org_id.clone(),
        action_type: envelope.action_type.clone(),
        resource: envelope.resource.clone(),
        estimated_cost_usd: format!("{:.6}", envelope.estimated_cost_usd),
        kernel_path: kernel_path.display().to_string(),
    })
}

pub fn run_supervisor(
    root: &Path,
    override_kernel_path: Option<&str>,
    max_jobs: usize,
) -> ShadowResult<SupervisorRunSummary> {
    let runtime_dir = ensure_runtime_dir(root)?;
    let queue_dir = runtime_dir.join("queue");
    let pending_dir = queue_dir.join("pending");
    let processed_dir = queue_dir.join("processed");
    let failed_dir = queue_dir.join("failed");
    fs::create_dir_all(&pending_dir).map_err(io_err)?;
    fs::create_dir_all(&processed_dir).map_err(io_err)?;
    fs::create_dir_all(&failed_dir).map_err(io_err)?;

    let mut pending = fs::read_dir(&pending_dir)
        .map_err(io_err)?
        .filter_map(|entry| entry.ok().map(|item| item.path()))
        .filter(|path| path.extension().map(|ext| ext == "json").unwrap_or(false))
        .collect::<Vec<_>>();
    pending.sort();

    let mut summary = SupervisorRunSummary {
        root: root.to_path_buf(),
        queue_dir: queue_dir.clone(),
        processed: 0,
        allowed: 0,
        denied: 0,
        failed: 0,
        last_input_hash: String::new(),
        last_execution_path: runtime_dir.join("last_execution.json"),
        audit_log_path: runtime_audit_log_path(root, override_kernel_path),
        note: "no queued actions were present".to_string(),
    };

    for path in pending.into_iter().take(max_jobs.max(1)) {
        let contents = fs::read_to_string(&path).map_err(io_err)?;
        let kernel_path_string = extract_json_string(&contents, "\"kernel_path\"")
            .filter(|value| !value.trim().is_empty())
            .or_else(|| override_kernel_path.map(|value| value.to_string()))
            .unwrap_or_else(|| kernel_path_for(root, override_kernel_path).map(|p| p.display().to_string()).unwrap_or_default());
        if kernel_path_string.is_empty() {
            return Err(format!(
                "queued action {} has no kernel_path and no override was provided",
                path.display()
            ));
        }
        let agent_id = extract_json_string(&contents, "\"agent_id\"")
            .ok_or_else(|| format!("agent_id missing in {}", path.display()))?;
        let org_id = extract_json_string(&contents, "\"org_id\"")
            .ok_or_else(|| format!("org_id missing in {}", path.display()))?;
        let action_type = extract_json_string(&contents, "\"action_type\"")
            .ok_or_else(|| format!("action_type missing in {}", path.display()))?;
        let resource = extract_json_string(&contents, "\"resource\"")
            .ok_or_else(|| format!("resource missing in {}", path.display()))?;
        let estimated_cost_usd = extract_json_number(&contents, "\"estimated_cost_usd\"")
            .ok_or_else(|| format!("estimated_cost_usd missing in {}", path.display()))?;
        let run_id = extract_json_string(&contents, "\"run_id\"").unwrap_or_default();
        let session_id = extract_json_string(&contents, "\"session_id\"").unwrap_or_default();

        let process_result = (|| -> ShadowResult<RuntimeExecutionCapture> {
            let identity = resolve_agent_identity(
                root,
                Some(&kernel_path_string),
                &agent_id,
                Some(&org_id),
            )?;
            let envelope = build_action_envelope(
                root,
                Some(&kernel_path_string),
                &agent_id,
                Some(&org_id),
                &action_type,
                &resource,
                estimated_cost_usd,
                if run_id.is_empty() { None } else { Some(run_id.as_str()) },
                if session_id.is_empty() {
                    None
                } else {
                    Some(session_id.as_str())
                },
            )?;
            let reference =
                evaluate_reference_gates(root, Some(&kernel_path_string), &identity, &envelope)?;
            let decision = capture_decision(root, &identity, &envelope, &reference)?;
            capture_runtime_execution(root, Path::new(&kernel_path_string), &envelope, &reference, &decision)
        })();

        match process_result {
            Ok(capture) => {
                summary.processed += 1;
                summary.last_input_hash = capture.input_hash.clone();
                summary.last_execution_path = capture.execution_path.clone();
                summary.audit_log_path = capture.audit_log_path.clone();
                if capture.overall_decision == "allow" {
                    summary.allowed += 1;
                } else {
                    summary.denied += 1;
                }
                let destination = processed_dir.join(
                    path.file_name()
                        .ok_or_else(|| format!("invalid queue file {}", path.display()))?,
                );
                fs::rename(&path, destination.clone()).map_err(io_err)?;
                let mut snapshot = read_job_snapshot(root, &capture.input_hash)
                    .unwrap_or_else(|_| JobSnapshot {
                        root: root.to_path_buf(),
                        job_id: capture.input_hash.clone(),
                        job_path: job_snapshot_path(root, &capture.input_hash),
                        status: "runtime_rehearsed".to_string(),
                        stage: "local_queue_supervisor".to_string(),
                        queue_bucket: "processed".to_string(),
                        queued_at: timestamp_now(),
                        updated_at: timestamp_now(),
                        agent_id: capture.agent_id.clone(),
                        org_id: capture.org_id.clone(),
                        action_type: capture.action_type.clone(),
                        resource: capture.resource.clone(),
                        estimated_cost_usd: format!("{:.6}", capture.estimated_cost_usd),
                        runtime_outcome: capture.runtime_outcome.clone(),
                        worker_status: capture.worker_status.clone(),
                        queue_path: None,
                        decision_path: Some(capture.decision_path.clone()),
                        execution_path: Some(capture.execution_path.clone()),
                        audit_log_path: Some(capture.audit_log_path.clone()),
                        parity_report_path: Some(capture.parity_report_path.clone()),
                        note: capture.worker_note.clone(),
                    });
                snapshot.queue_bucket = "processed".to_string();
                snapshot.stage = "local_queue_supervisor".to_string();
                snapshot.updated_at = timestamp_now();
                snapshot.queue_path = Some(destination);
                snapshot.decision_path = Some(capture.decision_path.clone());
                snapshot.execution_path = Some(capture.execution_path.clone());
                snapshot.audit_log_path = Some(capture.audit_log_path.clone());
                snapshot.parity_report_path = Some(capture.parity_report_path.clone());
                snapshot.runtime_outcome = capture.runtime_outcome.clone();
                snapshot.worker_status = capture.worker_status.clone();
                snapshot.status = if capture.overall_decision == "allow" {
                    if capture.worker_status == "completed" {
                        "completed".to_string()
                    } else {
                        "failed".to_string()
                    }
                } else {
                    "denied".to_string()
                };
                snapshot.note = capture.worker_note.clone();
                write_job_snapshot(root, snapshot)?;
            }
            Err(error) => {
                summary.failed += 1;
                let destination = failed_dir.join(
                    path.file_name()
                        .ok_or_else(|| format!("invalid queue file {}", path.display()))?,
                );
                let failure_payload = format!(
                    "{{\n  \"status\": \"failed\",\n  \"failed_at\": {},\n  \"source_path\": {},\n  \"error\": {},\n  \"queued_action\": {}\n}}\n",
                    json_string(&timestamp_now()),
                    json_string(&path.display().to_string()),
                    json_string(&error),
                    contents.trim(),
                );
                fs::write(&destination, failure_payload).map_err(io_err)?;
                fs::remove_file(&path).map_err(io_err)?;
                if let Some(input_hash) = extract_json_string(&contents, "\"input_hash\"") {
                    write_job_snapshot(
                        root,
                        JobSnapshot {
                            root: root.to_path_buf(),
                            job_id: input_hash.clone(),
                            job_path: job_snapshot_path(root, &input_hash),
                            status: "failed".to_string(),
                            stage: "local_queue_supervisor".to_string(),
                            queue_bucket: "failed".to_string(),
                            queued_at: extract_json_string(&contents, "\"queued_at\"")
                                .unwrap_or_else(timestamp_now),
                            updated_at: timestamp_now(),
                            agent_id: extract_json_string(&contents, "\"agent_id\"").unwrap_or_default(),
                            org_id: extract_json_string(&contents, "\"org_id\"").unwrap_or_default(),
                            action_type: extract_json_string(&contents, "\"action_type\"").unwrap_or_default(),
                            resource: extract_json_string(&contents, "\"resource\"").unwrap_or_default(),
                            estimated_cost_usd: extract_json_number(&contents, "\"estimated_cost_usd\"")
                                .map(|value| format!("{:.6}", value))
                                .unwrap_or_else(|| "0.000000".to_string()),
                            runtime_outcome: "supervisor_failed".to_string(),
                            worker_status: "failed_before_dispatch".to_string(),
                            queue_path: Some(destination),
                            decision_path: None,
                            execution_path: None,
                            audit_log_path: None,
                            parity_report_path: None,
                            note: error,
                        },
                    )?;
                }
            }
        }
    }

    if summary.processed > 0 || summary.failed > 0 {
        summary.note = format!(
            "processed={} allowed={} denied={} failed={}",
            summary.processed, summary.allowed, summary.denied, summary.failed
        );
    }
    Ok(summary)
}

pub fn list_jobs(
    root: &Path,
    status_filter: Option<&str>,
    limit: usize,
) -> ShadowResult<Vec<JobSnapshot>> {
    let jobs_dir = ensure_runtime_jobs_dir(root)?;
    let mut jobs = fs::read_dir(&jobs_dir)
        .map_err(io_err)?
        .filter_map(|entry| entry.ok().map(|item| item.path()))
        .filter(|path| path.is_dir())
        .filter_map(|path| {
            let job_id = path.file_name()?.to_string_lossy().to_string();
            read_job_snapshot(root, &job_id).ok()
        })
        .collect::<Vec<_>>();
    jobs.sort_by(|left, right| {
        right
            .updated_at
            .cmp(&left.updated_at)
            .then_with(|| right.job_id.cmp(&left.job_id))
    });
    if let Some(filter) = status_filter {
        jobs.retain(|job| job.status == filter);
    }
    if limit > 0 {
        jobs.truncate(limit);
    }
    Ok(jobs)
}

pub fn inspect_job(root: &Path, job_id: &str) -> ShadowResult<JobSnapshot> {
    read_job_snapshot(root, job_id)
}

pub fn watch_supervisor(
    root: &Path,
    override_kernel_path: Option<&str>,
    max_jobs: usize,
    iterations: usize,
    poll_seconds: u64,
) -> ShadowResult<SupervisorWatchSummary> {
    let runtime_dir = ensure_runtime_dir(root)?;
    let supervisor_dir = runtime_dir.join("supervisor");
    fs::create_dir_all(&supervisor_dir).map_err(io_err)?;
    let heartbeat_log_path = supervisor_dir.join("heartbeat.jsonl");
    let status_path = supervisor_dir.join("status.json");

    let mut summary = SupervisorWatchSummary {
        root: root.to_path_buf(),
        supervisor_dir,
        iterations,
        poll_seconds,
        processed: 0,
        allowed: 0,
        denied: 0,
        failed: 0,
        heartbeat_log_path: heartbeat_log_path.clone(),
        status_path: status_path.clone(),
        note: "no iterations executed".to_string(),
    };

    for iteration in 0..iterations.max(1) {
        let run = run_supervisor(root, override_kernel_path, max_jobs)?;
        summary.processed += run.processed;
        summary.allowed += run.allowed;
        summary.denied += run.denied;
        summary.failed += run.failed;
        summary.note = format!(
            "iterations={} processed={} allowed={} denied={} failed={}",
            iteration + 1,
            summary.processed,
            summary.allowed,
            summary.denied,
            summary.failed,
        );

        let pending_jobs = count_runtime_queue_entries(root, "pending")?;
        let processed_jobs = count_runtime_queue_entries(root, "processed")?;
        let failed_jobs = count_runtime_queue_entries(root, "failed")?;
        append_line(
            &heartbeat_log_path,
            &format!(
                "{{\"timestamp\":{},\"iteration\":{},\"processed\":{},\"allowed\":{},\"denied\":{},\"failed\":{},\"pending_jobs\":{},\"processed_jobs\":{},\"failed_jobs\":{},\"note\":{}}}\n",
                json_string(&timestamp_now()),
                iteration + 1,
                run.processed,
                run.allowed,
                run.denied,
                run.failed,
                pending_jobs,
                processed_jobs,
                failed_jobs,
                json_string(&run.note),
            ),
        )?;
        fs::write(
            &status_path,
            format!(
                "{{\n  \"status\": \"watch_complete\",\n  \"updated_at\": {},\n  \"iterations\": {},\n  \"poll_seconds\": {},\n  \"processed\": {},\n  \"allowed\": {},\n  \"denied\": {},\n  \"failed\": {},\n  \"pending_jobs\": {},\n  \"processed_jobs\": {},\n  \"failed_jobs\": {},\n  \"note\": {}\n}}\n",
                json_string(&timestamp_now()),
                iteration + 1,
                poll_seconds,
                summary.processed,
                summary.allowed,
                summary.denied,
                summary.failed,
                pending_jobs,
                processed_jobs,
                failed_jobs,
                json_string(&summary.note),
            ),
        )
        .map_err(io_err)?;

        if iteration + 1 < iterations.max(1) {
            thread::sleep(Duration::from_secs(poll_seconds));
        }
    }

    Ok(summary)
}

pub fn supervisor_status(root: &Path) -> ShadowResult<SupervisorStatusSnapshot> {
    let runtime_dir = ensure_runtime_dir(root)?;
    let supervisor_dir = runtime_dir.join("supervisor");
    fs::create_dir_all(&supervisor_dir).map_err(io_err)?;
    let status_path = supervisor_dir.join("status.json");
    let heartbeat_log_path = supervisor_dir.join("heartbeat.jsonl");

    if !status_path.exists() {
        return Ok(SupervisorStatusSnapshot {
            root: root.to_path_buf(),
            supervisor_dir,
            status_path,
            heartbeat_log_path,
            available: false,
            updated_at: "not_started".to_string(),
            iterations: 0,
            poll_seconds: 0,
            processed: 0,
            allowed: 0,
            denied: 0,
            failed: 0,
            pending_jobs: count_runtime_queue_entries(root, "pending")?,
            processed_jobs: count_runtime_queue_entries(root, "processed")?,
            failed_jobs: count_runtime_queue_entries(root, "failed")?,
            heartbeat_entries: 0,
            last_heartbeat_timestamp: "not_started".to_string(),
            note: "no supervisor status captured yet; run `loom supervisor watch` or `loom supervisor run` first".to_string(),
        });
    }

    let status_contents = fs::read_to_string(&status_path).map_err(io_err)?;
    let (heartbeat_entries, last_heartbeat_timestamp) = if heartbeat_log_path.exists() {
        let contents = fs::read_to_string(&heartbeat_log_path).map_err(io_err)?;
        let entries = contents
            .lines()
            .filter(|line| !line.trim().is_empty())
            .collect::<Vec<_>>();
        let last_timestamp = entries
            .last()
            .and_then(|line| extract_json_string(line, "\"timestamp\""))
            .unwrap_or_else(|| "unknown".to_string());
        (entries.len(), last_timestamp)
    } else {
        (0, "not_started".to_string())
    };

    Ok(SupervisorStatusSnapshot {
        root: root.to_path_buf(),
        supervisor_dir,
        status_path,
        heartbeat_log_path,
        available: true,
        updated_at: extract_json_string(&status_contents, "\"updated_at\"")
            .unwrap_or_else(|| "unknown".to_string()),
        iterations: extract_json_number(&status_contents, "\"iterations\"")
            .or_else(|| extract_json_number(&status_contents, "\"iteration\""))
            .map(|value| value as usize)
            .unwrap_or(0),
        poll_seconds: extract_json_number(&status_contents, "\"poll_seconds\"")
            .map(|value| value as u64)
            .unwrap_or(0),
        processed: extract_json_number(&status_contents, "\"processed\"")
            .map(|value| value as usize)
            .unwrap_or(0),
        allowed: extract_json_number(&status_contents, "\"allowed\"")
            .map(|value| value as usize)
            .unwrap_or(0),
        denied: extract_json_number(&status_contents, "\"denied\"")
            .map(|value| value as usize)
            .unwrap_or(0),
        failed: extract_json_number(&status_contents, "\"failed\"")
            .map(|value| value as usize)
            .unwrap_or(0),
        pending_jobs: extract_json_number(&status_contents, "\"pending_jobs\"")
            .map(|value| value as usize)
            .unwrap_or(0),
        processed_jobs: extract_json_number(&status_contents, "\"processed_jobs\"")
            .map(|value| value as usize)
            .unwrap_or(0),
        failed_jobs: extract_json_number(&status_contents, "\"failed_jobs\"")
            .map(|value| value as usize)
            .unwrap_or(0),
        heartbeat_entries,
        last_heartbeat_timestamp,
        note: extract_json_string(&status_contents, "\"note\"").unwrap_or_default(),
    })
}

pub fn run_supervisor_daemon_loop(
    root: &Path,
    override_kernel_path: Option<&str>,
    max_jobs: usize,
    poll_seconds: u64,
    max_iterations: usize,
    session_id: &str,
) -> ShadowResult<SupervisorDaemonSnapshot> {
    let runtime_dir = ensure_runtime_dir(root)?;
    let supervisor_dir = runtime_dir.join("supervisor");
    fs::create_dir_all(&supervisor_dir).map_err(io_err)?;
    let runtime_state_path = supervisor_dir.join("runtime_state.json");
    let stop_request_path = supervisor_dir.join("stop.requested");
    let heartbeat_log_path = supervisor_dir.join("heartbeat.jsonl");
    let status_path = supervisor_dir.join("status.json");
    let booted_at = timestamp_now();
    let pid = std::process::id();

    if stop_request_path.exists() {
        fs::remove_file(&stop_request_path).map_err(io_err)?;
    }

    let mut total_processed = 0usize;
    let mut total_allowed = 0usize;
    let mut total_denied = 0usize;
    let mut total_failed = 0usize;
    let mut iterations_completed = 0usize;
    let mut stop_reason = "max_iterations_completed".to_string();

    write_supervisor_runtime_state(
        &runtime_state_path,
        session_id,
        pid,
        true,
        "running",
        &booted_at,
        "",
        poll_seconds,
        max_jobs,
        max_iterations,
        iterations_completed,
        total_processed,
        total_allowed,
        total_denied,
        total_failed,
        count_runtime_queue_entries(root, "pending")?,
        count_runtime_queue_entries(root, "processed")?,
        count_runtime_queue_entries(root, "failed")?,
        format!("daemon session {} booted", session_id),
    )?;

    for iteration in 0..max_iterations.max(1) {
        if stop_request_path.exists() {
            stop_reason = "stop_requested".to_string();
            break;
        }

        let run = run_supervisor(root, override_kernel_path, max_jobs)?;
        iterations_completed = iteration + 1;
        total_processed += run.processed;
        total_allowed += run.allowed;
        total_denied += run.denied;
        total_failed += run.failed;

        let pending_jobs = count_runtime_queue_entries(root, "pending")?;
        let processed_jobs = count_runtime_queue_entries(root, "processed")?;
        let failed_jobs = count_runtime_queue_entries(root, "failed")?;

        append_line(
            &heartbeat_log_path,
            &format!(
                "{{\"timestamp\":{},\"mode\":\"daemon\",\"session_id\":{},\"iteration\":{},\"processed\":{},\"allowed\":{},\"denied\":{},\"failed\":{},\"pending_jobs\":{},\"processed_jobs\":{},\"failed_jobs\":{},\"note\":{}}}\n",
                json_string(&timestamp_now()),
                json_string(session_id),
                iterations_completed,
                run.processed,
                run.allowed,
                run.denied,
                run.failed,
                pending_jobs,
                processed_jobs,
                failed_jobs,
                json_string(&run.note),
            ),
        )?;

        fs::write(
            &status_path,
            format!(
                "{{\n  \"status\": \"daemon_iteration_complete\",\n  \"updated_at\": {},\n  \"session_id\": {},\n  \"pid\": {},\n  \"iteration\": {},\n  \"poll_seconds\": {},\n  \"processed\": {},\n  \"allowed\": {},\n  \"denied\": {},\n  \"failed\": {},\n  \"pending_jobs\": {},\n  \"processed_jobs\": {},\n  \"failed_jobs\": {},\n  \"note\": {}\n}}\n",
                json_string(&timestamp_now()),
                json_string(session_id),
                pid,
                iterations_completed,
                poll_seconds,
                total_processed,
                total_allowed,
                total_denied,
                total_failed,
                pending_jobs,
                processed_jobs,
                failed_jobs,
                json_string(&run.note),
            ),
        )
        .map_err(io_err)?;

        write_supervisor_runtime_state(
            &runtime_state_path,
            session_id,
            pid,
            true,
            "running",
            &booted_at,
            "",
            poll_seconds,
            max_jobs,
            max_iterations,
            iterations_completed,
            total_processed,
            total_allowed,
            total_denied,
            total_failed,
            pending_jobs,
            processed_jobs,
            failed_jobs,
            format!(
                "daemon iteration {} complete; processed={} allowed={} denied={} failed={}",
                iterations_completed, total_processed, total_allowed, total_denied, total_failed
            ),
        )?;

        if iterations_completed >= max_iterations {
            break;
        }
        if poll_seconds > 0 {
            thread::sleep(Duration::from_secs(poll_seconds));
        }
    }

    let stopped_at = timestamp_now();
    let pending_jobs = count_runtime_queue_entries(root, "pending")?;
    let processed_jobs = count_runtime_queue_entries(root, "processed")?;
    let failed_jobs = count_runtime_queue_entries(root, "failed")?;
    write_supervisor_runtime_state(
        &runtime_state_path,
        session_id,
        pid,
        false,
        if stop_reason == "stop_requested" {
            "stop_requested"
        } else {
            "completed"
        },
        &booted_at,
        &stopped_at,
        poll_seconds,
        max_jobs,
        max_iterations,
        iterations_completed,
        total_processed,
        total_allowed,
        total_denied,
        total_failed,
        pending_jobs,
        processed_jobs,
        failed_jobs,
        format!(
            "daemon session stopped via {} after {} iterations",
            stop_reason, iterations_completed
        ),
    )?;

    if stop_request_path.exists() {
        fs::remove_file(&stop_request_path).map_err(io_err)?;
    }

    supervisor_daemon_status(root)
}

pub fn supervisor_daemon_status(root: &Path) -> ShadowResult<SupervisorDaemonSnapshot> {
    let runtime_dir = ensure_runtime_dir(root)?;
    let supervisor_dir = runtime_dir.join("supervisor");
    fs::create_dir_all(&supervisor_dir).map_err(io_err)?;
    let runtime_state_path = supervisor_dir.join("runtime_state.json");
    let stop_request_path = supervisor_dir.join("stop.requested");
    let stdout_log_path = supervisor_dir.join("daemon.log");
    let heartbeat_log_path = supervisor_dir.join("heartbeat.jsonl");

    if !runtime_state_path.exists() {
        return Ok(SupervisorDaemonSnapshot {
            root: root.to_path_buf(),
            supervisor_dir,
            runtime_state_path,
            stop_request_path,
            stdout_log_path,
            available: false,
            session_id: String::new(),
            pid: 0,
            running: false,
            status: "not_started".to_string(),
            updated_at: "not_started".to_string(),
            booted_at: String::new(),
            stopped_at: String::new(),
            poll_seconds: 0,
            max_jobs: 0,
            max_iterations: 0,
            iterations_completed: 0,
            processed: 0,
            allowed: 0,
            denied: 0,
            failed: 0,
            pending_jobs: count_runtime_queue_entries(root, "pending")?,
            processed_jobs: count_runtime_queue_entries(root, "processed")?,
            failed_jobs: count_runtime_queue_entries(root, "failed")?,
            heartbeat_entries: 0,
            note: "no daemon state captured yet; run `loom supervisor daemon start` first".to_string(),
        });
    }

    let contents = fs::read_to_string(&runtime_state_path).map_err(io_err)?;
    let pid = extract_json_number(&contents, "\"pid\"")
        .map(|value| value as u32)
        .unwrap_or(0);
    let running_flag = extract_json_bool(&contents, "\"running\"").unwrap_or(false);
    let alive = if pid > 0 {
        PathBuf::from(format!("/proc/{}", pid)).exists()
    } else {
        false
    };
    let status = extract_json_string(&contents, "\"status\"").unwrap_or_else(|| "unknown".to_string());
    let (heartbeat_entries, _) = if heartbeat_log_path.exists() {
        let heartbeat = fs::read_to_string(&heartbeat_log_path).map_err(io_err)?;
        let entries = heartbeat.lines().filter(|line| !line.trim().is_empty()).count();
        (entries, ())
    } else {
        (0, ())
    };

    Ok(SupervisorDaemonSnapshot {
        root: root.to_path_buf(),
        supervisor_dir,
        runtime_state_path,
        stop_request_path,
        stdout_log_path,
        available: true,
        session_id: extract_json_string(&contents, "\"session_id\"").unwrap_or_default(),
        pid,
        running: running_flag && alive,
        status,
        updated_at: extract_json_string(&contents, "\"updated_at\"").unwrap_or_default(),
        booted_at: extract_json_string(&contents, "\"booted_at\"").unwrap_or_default(),
        stopped_at: extract_json_string(&contents, "\"stopped_at\"").unwrap_or_default(),
        poll_seconds: extract_json_number(&contents, "\"poll_seconds\"")
            .map(|value| value as u64)
            .unwrap_or(0),
        max_jobs: extract_json_number(&contents, "\"max_jobs\"")
            .map(|value| value as usize)
            .unwrap_or(0),
        max_iterations: extract_json_number(&contents, "\"max_iterations\"")
            .map(|value| value as usize)
            .unwrap_or(0),
        iterations_completed: extract_json_number(&contents, "\"iterations_completed\"")
            .map(|value| value as usize)
            .unwrap_or(0),
        processed: extract_json_number(&contents, "\"processed\"")
            .map(|value| value as usize)
            .unwrap_or(0),
        allowed: extract_json_number(&contents, "\"allowed\"")
            .map(|value| value as usize)
            .unwrap_or(0),
        denied: extract_json_number(&contents, "\"denied\"")
            .map(|value| value as usize)
            .unwrap_or(0),
        failed: extract_json_number(&contents, "\"failed\"")
            .map(|value| value as usize)
            .unwrap_or(0),
        pending_jobs: extract_json_number(&contents, "\"pending_jobs\"")
            .map(|value| value as usize)
            .unwrap_or(0),
        processed_jobs: extract_json_number(&contents, "\"processed_jobs\"")
            .map(|value| value as usize)
            .unwrap_or(0),
        failed_jobs: extract_json_number(&contents, "\"failed_jobs\"")
            .map(|value| value as usize)
            .unwrap_or(0),
        heartbeat_entries,
        note: extract_json_string(&contents, "\"note\"").unwrap_or_default(),
    })
}

pub fn request_supervisor_daemon_stop(root: &Path) -> ShadowResult<SupervisorDaemonSnapshot> {
    let runtime_dir = ensure_runtime_dir(root)?;
    let supervisor_dir = runtime_dir.join("supervisor");
    fs::create_dir_all(&supervisor_dir).map_err(io_err)?;
    let stop_request_path = supervisor_dir.join("stop.requested");
    fs::write(
        &stop_request_path,
        format!(
            "{{\n  \"status\": \"stop_requested\",\n  \"requested_at\": {}\n}}\n",
            json_string(&timestamp_now())
        ),
    )
    .map_err(io_err)?;
    let mut snapshot = supervisor_daemon_status(root)?;
    snapshot.note = format!(
        "{}; stop request recorded at {}",
        if snapshot.note.is_empty() {
            "stop requested".to_string()
        } else {
            snapshot.note.clone()
        },
        stop_request_path.display()
    );
    Ok(snapshot)
}

pub fn decision_exit_code(capture: &DecisionCapture, allow_code: i32, deny_code: i32) -> i32 {
    if capture.overall_decision == "allow" {
        allow_code
    } else {
        deny_code
    }
}

pub fn render_preflight_human(capture: &PreflightCapture) -> String {
    format!(
        "Meridian Loom // SHADOW PREFLIGHT\n===================================\nevent_log:              {}\naudit_preview_log:      {}\nreference_report:       {}\nreference_event_log:    {}\nlatest_report:          {}\ninput_hash:             {}\nestimated_cost_usd:     {:.4}\nidentity_restrictions:  {}\nreference_restrictions: {}\nsanction_controls:      {} (snapshot: {})\nbudget_limit_usd:       {}\nbudget_gate:            {}\napproval_hook:          {} (policy: {})\naudit_emission:         {}\noverall_decision:       {}\nreference_stage:        {}\nreference_reason:       {}\ncaptured_hooks:         {}\n",
        capture.event_log.display(),
        capture.audit_preview_log.display(),
        capture.reference_report.display(),
        capture.reference_event_log.display(),
        capture.latest_report.display(),
        capture.input_hash,
        capture.estimated_cost_usd,
        if capture.identity_restrictions.is_empty() {
            "(none)".to_string()
        } else {
            capture.identity_restrictions.join(", ")
        },
        if capture.reference_restrictions.is_empty() {
            "(none)".to_string()
        } else {
            capture.reference_restrictions.join(", ")
        },
        capture.sanction_gate_decision,
        capture.sanction_decision,
        capture
            .budget_limit_usd
            .map(|value| format!("{:.4}", value))
            .unwrap_or_else(|| "(unknown)".to_string()),
        capture.budget_gate_decision,
        capture.approval_gate_decision,
        capture.approval_decision,
        capture.audit_emission_decision,
        capture.overall_decision,
        capture.reference_stage,
        if capture.reference_reason.is_empty() {
            "(none)"
        } else {
            &capture.reference_reason
        },
        capture.hooks.join(", ")
    )
}

pub fn render_preflight_json(capture: &PreflightCapture) -> String {
    format!(
        "{{\n  \"event_log\": {},\n  \"audit_preview_log\": {},\n  \"reference_report\": {},\n  \"reference_event_log\": {},\n  \"latest_report\": {},\n  \"input_hash\": {},\n  \"estimated_cost_usd\": {:.6},\n  \"identity_restrictions\": {},\n  \"reference_restrictions\": {},\n  \"sanction_decision\": {},\n  \"sanction_gate_decision\": {},\n  \"budget_limit_usd\": {},\n  \"budget_gate_decision\": {},\n  \"approval_decision\": {},\n  \"approval_gate_decision\": {},\n  \"audit_emission_decision\": {},\n  \"overall_decision\": {},\n  \"reference_stage\": {},\n  \"reference_reason\": {},\n  \"captured_hooks\": [\"agent_identity\", \"action_envelope\", \"cost_attribution\", \"approval_hook\", \"audit_emission\", \"sanction_controls\", \"budget_gate\"]\n}}\n",
        json_string(&capture.event_log.display().to_string()),
        json_string(&capture.audit_preview_log.display().to_string()),
        json_string(&capture.reference_report.display().to_string()),
        json_string(&capture.reference_event_log.display().to_string()),
        json_string(&capture.latest_report.display().to_string()),
        json_string(&capture.input_hash),
        capture.estimated_cost_usd,
        render_json_string_array(&capture.identity_restrictions),
        render_json_string_array(&capture.reference_restrictions),
        json_string(&capture.sanction_decision),
        json_string(&capture.sanction_gate_decision),
        capture
            .budget_limit_usd
            .map(|value| format!("{:.6}", value))
            .unwrap_or_else(|| "null".to_string()),
        json_string(&capture.budget_gate_decision),
        json_string(&capture.approval_decision),
        json_string(&capture.approval_gate_decision),
        json_string(&capture.audit_emission_decision),
        json_string(&capture.overall_decision),
        json_string(&capture.reference_stage),
        json_string(&capture.reference_reason),
    )
}

pub fn render_decision_human(capture: &DecisionCapture) -> String {
    format!(
        "Meridian Loom // SHADOW DECISION\n==================================\ndecision_path:          {}\nsource:                 {}\nagent_id:               {}\norg_id:                 {}\naction_type:            {}\nresource:               {}\ninput_hash:             {}\nestimated_cost_usd:{:>12.4}\nidentity_restrictions:  {}\nreference_restrictions: {}\nlocal_sanction:         {} ({})\nsanction_gate:          {}\napproval_gate:          {}\nbudget_limit_usd:       {}\nbudget_gate:            {}\noverall_decision:       {}\neffective_source:       {}\neffective_stage:        {}\neffective_reason:       {}\nreference_stage:        {}\nreference_reason:       {}\n",
        capture.decision_path.display(),
        capture.source,
        capture.agent_id,
        capture.org_id,
        capture.action_type,
        capture.resource,
        capture.input_hash,
        capture.estimated_cost_usd,
        if capture.identity_restrictions.is_empty() {
            "(none)".to_string()
        } else {
            capture.identity_restrictions.join(", ")
        },
        if capture.reference_restrictions.is_empty() {
            "(none)".to_string()
        } else {
            capture.reference_restrictions.join(", ")
        },
        capture.local_sanction_decision,
        capture.local_sanction_reason,
        capture.sanction_gate_decision,
        capture.approval_gate_decision,
        capture
            .budget_limit_usd
            .map(|value| format!("{:.4}", value))
            .unwrap_or_else(|| "(unknown)".to_string()),
        capture.budget_gate_decision,
        capture.overall_decision,
        capture.effective_source,
        capture.effective_stage,
        if capture.effective_reason.is_empty() {
            "(none)"
        } else {
            &capture.effective_reason
        },
        capture.reference_stage,
        if capture.reference_reason.is_empty() {
            "(none)"
        } else {
            &capture.reference_reason
        },
    )
}

pub fn render_decision_json(capture: &DecisionCapture) -> String {
    format!(
        "{{\n  \"status\": \"decision_captured\",\n  \"decision_path\": {},\n  \"source\": {},\n  \"agent_id\": {},\n  \"org_id\": {},\n  \"action_type\": {},\n  \"resource\": {},\n  \"input_hash\": {},\n  \"estimated_cost_usd\": {:.6},\n  \"identity_restrictions\": {},\n  \"reference_restrictions\": {},\n  \"local_sanction_allowed\": {},\n  \"local_sanction_decision\": {},\n  \"local_sanction_reason\": {},\n  \"sanction_gate_decision\": {},\n  \"approval_gate_decision\": {},\n  \"budget_limit_usd\": {},\n  \"budget_gate_decision\": {},\n  \"overall_decision\": {},\n  \"effective_source\": {},\n  \"effective_stage\": {},\n  \"effective_reason\": {},\n  \"reference_stage\": {},\n  \"reference_reason\": {},\n  \"note\": \"experimental preflight decision only; not governed runtime enforcement\"\n}}\n",
        json_string(&capture.decision_path.display().to_string()),
        json_string(&capture.source),
        json_string(&capture.agent_id),
        json_string(&capture.org_id),
        json_string(&capture.action_type),
        json_string(&capture.resource),
        json_string(&capture.input_hash),
        capture.estimated_cost_usd,
        render_json_string_array(&capture.identity_restrictions),
        render_json_string_array(&capture.reference_restrictions),
        if capture.local_sanction_allowed { "true" } else { "false" },
        json_string(&capture.local_sanction_decision),
        json_string(&capture.local_sanction_reason),
        json_string(&capture.sanction_gate_decision),
        json_string(&capture.approval_gate_decision),
        capture
            .budget_limit_usd
            .map(|value| format!("{:.6}", value))
            .unwrap_or_else(|| "null".to_string()),
        json_string(&capture.budget_gate_decision),
        json_string(&capture.overall_decision),
        json_string(&capture.effective_source),
        json_string(&capture.effective_stage),
        json_string(&capture.effective_reason),
        json_string(&capture.reference_stage),
        json_string(&capture.reference_reason),
    )
}

pub fn render_runtime_execution_human(capture: &RuntimeExecutionCapture) -> String {
    let root = capture
        .execution_path
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "(unknown)".to_string());
    format!(
        "Meridian Loom // RUNTIME EXECUTE\n=================================\nphase:       experimental runtime rehearsal\nboundary:    local governed supervisor path is real; hosted runtime replacement is not\n\nDecision\n========\nagent_id:            {}\norg_id:              {}\naction_type:         {}\nresource:            {}\ninput_hash:          {}\nestimated_cost_usd:  {:.4}\noverall_decision:    {}\neffective_source:    {}\neffective_stage:     {}\nreference_decision:  {}\nreference_stage:     {}\nruntime_outcome:     {}\nworker_status:       {}\nworker_kind:         {}\nworker_note:         {}\nparity_status:       {}\nparity_reason:       {}\n\nworker supervisor artifacts\n===========================\nworker_request:      {}\nworker_result:       {}\nworker_log:          {}\n\naudit / parity artifacts\n========================\nexecution_path:      {}\ndecision_path:       {}\naudit_log:           {} ({})\nparity_stream:       {}\nparity_report:       {}\nopenclaw_live_probe: {} ({})\nopenclaw_probe_log:  {}\n\nNext\n====\n1. loom job inspect --job-id {} --root {}\n2. loom parity report --root {}\n3. loom shadow report --root {}\n4. Inspect {} for worker execution details.\n5. Inspect {} for runtime-side audit details.\n",
        capture.agent_id,
        capture.org_id,
        capture.action_type,
        capture.resource,
        capture.input_hash,
        capture.estimated_cost_usd,
        capture.overall_decision,
        capture.effective_source,
        capture.effective_stage,
        capture.reference_decision,
        capture.reference_stage,
        capture.runtime_outcome,
        capture.worker_status,
        capture.worker_kind,
        capture.worker_note,
        capture.parity_status,
        capture.parity_reason,
        capture.worker_request_path.display(),
        capture.worker_result_path.display(),
        capture.worker_log_path.display(),
        capture.execution_path.display(),
        capture.decision_path.display(),
        capture.audit_log_path.display(),
        capture.audit_emission_status,
        capture.parity_stream_path.display(),
        capture.parity_report_path.display(),
        capture
            .openclaw_live_probe_path
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "(not captured)".to_string()),
        capture.openclaw_live_probe_status,
        capture
            .openclaw_live_probe_stream_path
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "(not captured)".to_string()),
        capture.input_hash,
        root,
        root,
        root,
        capture.worker_log_path.display(),
        capture.audit_log_path.display(),
    )
}

pub fn render_runtime_execution_json(capture: &RuntimeExecutionCapture) -> String {
    format!(
        "{{\n  \"status\": \"runtime_execution_captured\",\n  \"execution_path\": {},\n  \"worker_request_path\": {},\n  \"worker_result_path\": {},\n  \"worker_log_path\": {},\n  \"decision_path\": {},\n  \"audit_log_path\": {},\n  \"parity_stream_path\": {},\n  \"parity_report_path\": {},\n  \"openclaw_live_probe_path\": {},\n  \"openclaw_live_probe_stream_path\": {},\n  \"agent_id\": {},\n  \"org_id\": {},\n  \"action_type\": {},\n  \"resource\": {},\n  \"input_hash\": {},\n  \"estimated_cost_usd\": {:.6},\n  \"runtime_outcome\": {},\n  \"worker_status\": {},\n  \"worker_kind\": {},\n  \"worker_note\": {},\n  \"overall_decision\": {},\n  \"effective_source\": {},\n  \"effective_stage\": {},\n  \"reference_decision\": {},\n  \"reference_stage\": {},\n  \"audit_emission_status\": {},\n  \"openclaw_live_probe_status\": {},\n  \"openclaw_live_probe_note\": {},\n  \"parity_status\": {},\n  \"parity_reason\": {},\n  \"note\": \"experimental local supervisor path exists for allow decisions; governed hosted supervisor remains future work\"\n}}\n",
        json_string(&capture.execution_path.display().to_string()),
        json_string(&capture.worker_request_path.display().to_string()),
        json_string(&capture.worker_result_path.display().to_string()),
        json_string(&capture.worker_log_path.display().to_string()),
        json_string(&capture.decision_path.display().to_string()),
        json_string(&capture.audit_log_path.display().to_string()),
        json_string(&capture.parity_stream_path.display().to_string()),
        json_string(&capture.parity_report_path.display().to_string()),
        capture
            .openclaw_live_probe_path
            .as_ref()
            .map(|path| json_string(&path.display().to_string()))
            .unwrap_or_else(|| "null".to_string()),
        capture
            .openclaw_live_probe_stream_path
            .as_ref()
            .map(|path| json_string(&path.display().to_string()))
            .unwrap_or_else(|| "null".to_string()),
        json_string(&capture.agent_id),
        json_string(&capture.org_id),
        json_string(&capture.action_type),
        json_string(&capture.resource),
        json_string(&capture.input_hash),
        capture.estimated_cost_usd,
        json_string(&capture.runtime_outcome),
        json_string(&capture.worker_status),
        json_string(&capture.worker_kind),
        json_string(&capture.worker_note),
        json_string(&capture.overall_decision),
        json_string(&capture.effective_source),
        json_string(&capture.effective_stage),
        json_string(&capture.reference_decision),
        json_string(&capture.reference_stage),
        json_string(&capture.audit_emission_status),
        json_string(&capture.openclaw_live_probe_status),
        json_string(&capture.openclaw_live_probe_note),
        json_string(&capture.parity_status),
        json_string(&capture.parity_reason),
    )
}

pub fn render_enqueued_action_human(capture: &EnqueuedAction) -> String {
    format!(
        "Meridian Loom // ACTION ENQUEUED\n=================================\nqueue_path:           {}\njob_path:             {}\ninput_hash:           {}\nagent_id:             {}\norg_id:               {}\naction_type:          {}\nresource:             {}\nestimated_cost_usd:   {}\nkernel_path:          {}\nnext_step:            loom job inspect --job-id {} --root <path>\nthen:                 loom supervisor run --root <path> --max-jobs 1\n",
        capture.queue_path.display(),
        capture.job_path.display(),
        capture.input_hash,
        capture.agent_id,
        capture.org_id,
        capture.action_type,
        capture.resource,
        capture.estimated_cost_usd,
        capture.kernel_path,
        capture.input_hash,
    )
}

pub fn render_enqueued_action_json(capture: &EnqueuedAction) -> String {
    format!(
        "{{\n  \"status\": \"queued\",\n  \"queue_path\": {},\n  \"job_path\": {},\n  \"input_hash\": {},\n  \"agent_id\": {},\n  \"org_id\": {},\n  \"action_type\": {},\n  \"resource\": {},\n  \"estimated_cost_usd\": {},\n  \"kernel_path\": {}\n}}\n",
        json_string(&capture.queue_path.display().to_string()),
        json_string(&capture.job_path.display().to_string()),
        json_string(&capture.input_hash),
        json_string(&capture.agent_id),
        json_string(&capture.org_id),
        json_string(&capture.action_type),
        json_string(&capture.resource),
        capture.estimated_cost_usd,
        json_string(&capture.kernel_path),
    )
}

pub fn render_job_list_human(root: &Path, jobs: &[JobSnapshot], status_filter: Option<&str>) -> String {
    let entries = if jobs.is_empty() {
        "  (none)\n".to_string()
    } else {
        jobs.iter()
            .map(|job| {
                format!(
                    "  {} | status={} | stage={} | bucket={} | agent={} | action={}::{} | updated_at={}\n",
                    job.job_id,
                    job.status,
                    job.stage,
                    job.queue_bucket,
                    job.agent_id,
                    job.action_type,
                    job.resource,
                    job.updated_at
                )
            })
            .collect::<String>()
    };
    format!(
        "Meridian Loom // JOB LIST\n==========================\nphase:       experimental runtime-owned job ledger\nboundary:    local job state is real; hosted scheduler remains future work\n\nCurrent state\n=============\nroot:                {}\nstatus_filter:       {}\njobs_found:          {}\n\nEntries\n-------\n{}\
\nNext\n====\n1. loom job inspect --job-id <input_hash> --root {}\n2. loom supervisor daemon status --root {}\n3. loom parity report --root {}\n",
        root.display(),
        status_filter.unwrap_or("(none)"),
        jobs.len(),
        entries,
        root.display(),
        root.display(),
        root.display(),
    )
}

pub fn render_job_list_json(jobs: &[JobSnapshot]) -> String {
    let rendered = jobs
        .iter()
        .map(render_job_snapshot_json)
        .collect::<Vec<_>>()
        .join(",\n");
    format!(
        "{{\n  \"status\": \"job_list\",\n  \"jobs\": [\n{}\n  ]\n}}\n",
        rendered
    )
}

pub fn render_job_inspect_human(job: &JobSnapshot) -> String {
    format!(
        "Meridian Loom // JOB INSPECT\n=============================\nphase:       experimental runtime-owned job ledger\nboundary:    job lifecycle is locally inspectable; hosted scheduler remains future work\n\nCurrent state\n=============\njob_id:              {}\nstatus:              {}\nstage:               {}\nqueue_bucket:        {}\nqueued_at:           {}\nupdated_at:          {}\nagent_id:            {}\norg_id:              {}\naction_type:         {}\nresource:            {}\nestimated_cost_usd:  {}\nruntime_outcome:     {}\nworker_status:       {}\nnote:                {}\n\nArtifacts\n=========\njob_path:            {}\nqueue_path:          {}\ndecision_path:       {}\nexecution_path:      {}\naudit_log_path:      {}\nparity_report_path:  {}\n\nNext\n====\n1. loom parity report --root {}\n2. loom shadow report --root {}\n3. Inspect {} for the latest persisted job state.\n",
        job.job_id,
        job.status,
        job.stage,
        job.queue_bucket,
        job.queued_at,
        job.updated_at,
        job.agent_id,
        job.org_id,
        job.action_type,
        job.resource,
        job.estimated_cost_usd,
        job.runtime_outcome,
        job.worker_status,
        job.note,
        job.job_path.display(),
        display_optional_path(job.queue_path.as_ref()),
        display_optional_path(job.decision_path.as_ref()),
        display_optional_path(job.execution_path.as_ref()),
        display_optional_path(job.audit_log_path.as_ref()),
        display_optional_path(job.parity_report_path.as_ref()),
        job.root.display(),
        job.root.display(),
        job.job_path.display(),
    )
}

pub fn render_job_inspect_json(job: &JobSnapshot) -> String {
    format!(
        "{{\n  \"status\": \"job_snapshot\",\n  {}\n}}\n",
        render_job_snapshot_json(job)
            .trim()
            .trim_start_matches('{')
            .trim_end_matches('}')
    )
}

pub fn render_supervisor_run_human(summary: &SupervisorRunSummary) -> String {
    format!(
        "Meridian Loom // SUPERVISOR RUN\n================================\nphase:       experimental local queue supervisor\nboundary:    queued worker dispatch is real; hosted governed supervisor is not\n\nCurrent state\n=============\nroot:                {}\nqueue_dir:           {}\nprocessed:           {}\nallowed:             {}\ndenied:              {}\nfailed:              {}\nlast_input_hash:     {}\nlast_execution:      {}\naudit_log:           {}\nnote:                {}\n\nNext\n====\n1. loom parity report --root {}\n2. loom shadow report --root {}\n3. Inspect {} for canonical runtime audit entries.\n",
        summary.root.display(),
        summary.queue_dir.display(),
        summary.processed,
        summary.allowed,
        summary.denied,
        summary.failed,
        if summary.last_input_hash.is_empty() {
            "(none)".to_string()
        } else {
            summary.last_input_hash.clone()
        },
        summary.last_execution_path.display(),
        summary.audit_log_path.display(),
        summary.note,
        summary.root.display(),
        summary.root.display(),
        summary.audit_log_path.display(),
    )
}

pub fn render_supervisor_run_json(summary: &SupervisorRunSummary) -> String {
    format!(
        "{{\n  \"status\": \"supervisor_run_complete\",\n  \"root\": {},\n  \"queue_dir\": {},\n  \"processed\": {},\n  \"allowed\": {},\n  \"denied\": {},\n  \"failed\": {},\n  \"last_input_hash\": {},\n  \"last_execution_path\": {},\n  \"audit_log_path\": {},\n  \"note\": {}\n}}\n",
        json_string(&summary.root.display().to_string()),
        json_string(&summary.queue_dir.display().to_string()),
        summary.processed,
        summary.allowed,
        summary.denied,
        summary.failed,
        json_string(&summary.last_input_hash),
        json_string(&summary.last_execution_path.display().to_string()),
        json_string(&summary.audit_log_path.display().to_string()),
        json_string(&summary.note),
    )
}

pub fn render_supervisor_watch_human(summary: &SupervisorWatchSummary) -> String {
    format!(
        "Meridian Loom // SUPERVISOR WATCH\n==================================\nphase:       experimental local queue supervisor loop\nboundary:    heartbeat and queue watch are real; hosted scheduler remains future work\n\nCurrent state\n=============\nroot:                {}\nsupervisor_dir:      {}\niterations:          {}\npoll_seconds:        {}\nprocessed:           {}\nallowed:             {}\ndenied:              {}\nfailed:              {}\nheartbeat_log:       {}\nstatus_path:         {}\nnote:                {}\n\nNext\n====\n1. loom supervisor run --root {} --max-jobs 1\n2. loom parity report --root {}\n3. inspect {} for supervisor heartbeat history.\n",
        summary.root.display(),
        summary.supervisor_dir.display(),
        summary.iterations,
        summary.poll_seconds,
        summary.processed,
        summary.allowed,
        summary.denied,
        summary.failed,
        summary.heartbeat_log_path.display(),
        summary.status_path.display(),
        summary.note,
        summary.root.display(),
        summary.root.display(),
        summary.heartbeat_log_path.display(),
    )
}

pub fn render_supervisor_watch_json(summary: &SupervisorWatchSummary) -> String {
    format!(
        "{{\n  \"status\": \"supervisor_watch_complete\",\n  \"root\": {},\n  \"supervisor_dir\": {},\n  \"iterations\": {},\n  \"poll_seconds\": {},\n  \"processed\": {},\n  \"allowed\": {},\n  \"denied\": {},\n  \"failed\": {},\n  \"heartbeat_log_path\": {},\n  \"status_path\": {},\n  \"note\": {}\n}}\n",
        json_string(&summary.root.display().to_string()),
        json_string(&summary.supervisor_dir.display().to_string()),
        summary.iterations,
        summary.poll_seconds,
        summary.processed,
        summary.allowed,
        summary.denied,
        summary.failed,
        json_string(&summary.heartbeat_log_path.display().to_string()),
        json_string(&summary.status_path.display().to_string()),
        json_string(&summary.note),
    )
}

pub fn render_supervisor_status_human(snapshot: &SupervisorStatusSnapshot) -> String {
    if !snapshot.available {
        return format!(
            "Meridian Loom // SUPERVISOR STATUS\n===================================\nphase:       experimental local queue supervisor loop\nboundary:    no watch status captured yet; hosted scheduler remains future work\n\nCurrent state\n=============\nroot:                {}\nsupervisor_dir:      {}\nstatus_path:         {}\nheartbeat_log:       {}\npending_jobs:        {}\nprocessed_jobs:      {}\nfailed_jobs:         {}\nheartbeat_entries:   {}\nnote:                {}\n\nNext\n====\n1. loom supervisor watch --root {} --max-jobs 1 --iterations 2 --poll-seconds 0\n2. loom supervisor run --root {} --max-jobs 1\n3. inspect {} after a watch loop has executed.\n",
            snapshot.root.display(),
            snapshot.supervisor_dir.display(),
            snapshot.status_path.display(),
            snapshot.heartbeat_log_path.display(),
            snapshot.pending_jobs,
            snapshot.processed_jobs,
            snapshot.failed_jobs,
            snapshot.heartbeat_entries,
            snapshot.note,
            snapshot.root.display(),
            snapshot.root.display(),
            snapshot.status_path.display(),
        );
    }

    format!(
        "Meridian Loom // SUPERVISOR STATUS\n===================================\nphase:       experimental local queue supervisor loop\nboundary:    bounded local loop state is real; hosted scheduler remains future work\n\nCurrent state\n=============\nroot:                {}\nsupervisor_dir:      {}\nupdated_at:          {}\niterations:          {}\npoll_seconds:        {}\nprocessed:           {}\nallowed:             {}\ndenied:              {}\nfailed:              {}\npending_jobs:        {}\nprocessed_jobs:      {}\nfailed_jobs:         {}\nheartbeat_entries:   {}\nlast_heartbeat:      {}\nstatus_path:         {}\nheartbeat_log:       {}\nnote:                {}\n\nNext\n====\n1. loom parity report --root {}\n2. loom shadow report --root {}\n3. inspect {} for the latest heartbeat history.\n",
        snapshot.root.display(),
        snapshot.supervisor_dir.display(),
        snapshot.updated_at,
        snapshot.iterations,
        snapshot.poll_seconds,
        snapshot.processed,
        snapshot.allowed,
        snapshot.denied,
        snapshot.failed,
        snapshot.pending_jobs,
        snapshot.processed_jobs,
        snapshot.failed_jobs,
        snapshot.heartbeat_entries,
        snapshot.last_heartbeat_timestamp,
        snapshot.status_path.display(),
        snapshot.heartbeat_log_path.display(),
        snapshot.note,
        snapshot.root.display(),
        snapshot.root.display(),
        snapshot.heartbeat_log_path.display(),
    )
}

pub fn render_supervisor_status_json(snapshot: &SupervisorStatusSnapshot) -> String {
    format!(
        "{{\n  \"status\": {},\n  \"root\": {},\n  \"supervisor_dir\": {},\n  \"status_path\": {},\n  \"heartbeat_log_path\": {},\n  \"available\": {},\n  \"updated_at\": {},\n  \"iterations\": {},\n  \"poll_seconds\": {},\n  \"processed\": {},\n  \"allowed\": {},\n  \"denied\": {},\n  \"failed\": {},\n  \"pending_jobs\": {},\n  \"processed_jobs\": {},\n  \"failed_jobs\": {},\n  \"heartbeat_entries\": {},\n  \"last_heartbeat_timestamp\": {},\n  \"note\": {}\n}}\n",
        json_string(if snapshot.available {
            "supervisor_status_available"
        } else {
            "supervisor_status_not_started"
        }),
        json_string(&snapshot.root.display().to_string()),
        json_string(&snapshot.supervisor_dir.display().to_string()),
        json_string(&snapshot.status_path.display().to_string()),
        json_string(&snapshot.heartbeat_log_path.display().to_string()),
        if snapshot.available { "true" } else { "false" },
        json_string(&snapshot.updated_at),
        snapshot.iterations,
        snapshot.poll_seconds,
        snapshot.processed,
        snapshot.allowed,
        snapshot.denied,
        snapshot.failed,
        snapshot.pending_jobs,
        snapshot.processed_jobs,
        snapshot.failed_jobs,
        snapshot.heartbeat_entries,
        json_string(&snapshot.last_heartbeat_timestamp),
        json_string(&snapshot.note),
    )
}

pub fn render_supervisor_daemon_human(snapshot: &SupervisorDaemonSnapshot) -> String {
    if !snapshot.available {
        return format!(
            "Meridian Loom // SUPERVISOR DAEMON\n===================================\nphase:       experimental local queue supervisor daemon rehearsal\nboundary:    no daemon state captured yet; hosted scheduler remains future work\n\nCurrent state\n=============\nroot:                {}\nsupervisor_dir:      {}\nruntime_state:       {}\nstdout_log:          {}\npending_jobs:        {}\nprocessed_jobs:      {}\nfailed_jobs:         {}\nheartbeat_entries:   {}\nnote:                {}\n\nNext\n====\n1. loom supervisor daemon start --root {} --max-jobs 1 --poll-seconds 1 --iterations 10\n2. loom supervisor status --root {}\n3. inspect {} once the daemon has started.\n",
            snapshot.root.display(),
            snapshot.supervisor_dir.display(),
            snapshot.runtime_state_path.display(),
            snapshot.stdout_log_path.display(),
            snapshot.pending_jobs,
            snapshot.processed_jobs,
            snapshot.failed_jobs,
            snapshot.heartbeat_entries,
            snapshot.note,
            snapshot.root.display(),
            snapshot.root.display(),
            snapshot.runtime_state_path.display(),
        );
    }

    format!(
        "Meridian Loom // SUPERVISOR DAEMON\n===================================\nphase:       experimental local queue supervisor daemon rehearsal\nboundary:    daemon lifecycle is locally real; hosted scheduler remains future work\n\nCurrent state\n=============\nroot:                {}\nsupervisor_dir:      {}\nsession_id:          {}\npid:                 {}\nrunning:             {}\nstatus:              {}\nbooted_at:           {}\nupdated_at:          {}\nstopped_at:          {}\npoll_seconds:        {}\nmax_jobs:            {}\nmax_iterations:      {}\niterations_completed:{}\nprocessed:           {}\nallowed:             {}\ndenied:              {}\nfailed:              {}\npending_jobs:        {}\nprocessed_jobs:      {}\nfailed_jobs:         {}\nheartbeat_entries:   {}\nruntime_state:       {}\nstdout_log:          {}\nstop_request:        {}\nnote:                {}\n\nNext\n====\n1. loom supervisor daemon stop --root {}\n2. loom supervisor status --root {}\n3. inspect {} for background daemon output.\n",
        snapshot.root.display(),
        snapshot.supervisor_dir.display(),
        if snapshot.session_id.is_empty() { "(none)".to_string() } else { snapshot.session_id.clone() },
        snapshot.pid,
        if snapshot.running { "true" } else { "false" },
        snapshot.status,
        if snapshot.booted_at.is_empty() { "(none)".to_string() } else { snapshot.booted_at.clone() },
        if snapshot.updated_at.is_empty() { "(none)".to_string() } else { snapshot.updated_at.clone() },
        if snapshot.stopped_at.is_empty() { "(none)".to_string() } else { snapshot.stopped_at.clone() },
        snapshot.poll_seconds,
        snapshot.max_jobs,
        snapshot.max_iterations,
        snapshot.iterations_completed,
        snapshot.processed,
        snapshot.allowed,
        snapshot.denied,
        snapshot.failed,
        snapshot.pending_jobs,
        snapshot.processed_jobs,
        snapshot.failed_jobs,
        snapshot.heartbeat_entries,
        snapshot.runtime_state_path.display(),
        snapshot.stdout_log_path.display(),
        snapshot.stop_request_path.display(),
        snapshot.note,
        snapshot.root.display(),
        snapshot.root.display(),
        snapshot.stdout_log_path.display(),
    )
}

pub fn render_supervisor_daemon_json(snapshot: &SupervisorDaemonSnapshot) -> String {
    format!(
        "{{\n  \"status\": {},\n  \"root\": {},\n  \"supervisor_dir\": {},\n  \"runtime_state_path\": {},\n  \"stop_request_path\": {},\n  \"stdout_log_path\": {},\n  \"available\": {},\n  \"session_id\": {},\n  \"pid\": {},\n  \"running\": {},\n  \"daemon_status\": {},\n  \"updated_at\": {},\n  \"booted_at\": {},\n  \"stopped_at\": {},\n  \"poll_seconds\": {},\n  \"max_jobs\": {},\n  \"max_iterations\": {},\n  \"iterations_completed\": {},\n  \"processed\": {},\n  \"allowed\": {},\n  \"denied\": {},\n  \"failed\": {},\n  \"pending_jobs\": {},\n  \"processed_jobs\": {},\n  \"failed_jobs\": {},\n  \"heartbeat_entries\": {},\n  \"note\": {}\n}}\n",
        json_string(if snapshot.available {
            "supervisor_daemon_status_available"
        } else {
            "supervisor_daemon_not_started"
        }),
        json_string(&snapshot.root.display().to_string()),
        json_string(&snapshot.supervisor_dir.display().to_string()),
        json_string(&snapshot.runtime_state_path.display().to_string()),
        json_string(&snapshot.stop_request_path.display().to_string()),
        json_string(&snapshot.stdout_log_path.display().to_string()),
        if snapshot.available { "true" } else { "false" },
        json_string(&snapshot.session_id),
        snapshot.pid,
        if snapshot.running { "true" } else { "false" },
        json_string(&snapshot.status),
        json_string(&snapshot.updated_at),
        json_string(&snapshot.booted_at),
        json_string(&snapshot.stopped_at),
        snapshot.poll_seconds,
        snapshot.max_jobs,
        snapshot.max_iterations,
        snapshot.iterations_completed,
        snapshot.processed,
        snapshot.allowed,
        snapshot.denied,
        snapshot.failed,
        snapshot.pending_jobs,
        snapshot.processed_jobs,
        snapshot.failed_jobs,
        snapshot.heartbeat_entries,
        json_string(&snapshot.note),
    )
}

pub fn render_compare_human(summary: &ComparisonSummary) -> String {
    let divergence_lines = {
        let rendered = summary
            .hook_results
            .iter()
            .filter(|item| !item.matched)
            .map(|item| {
                format!(
                    "  [{}] {} | primary={} | shadow={} | input={}\n",
                    item.pair_index,
                    item.hook_name,
                    item.primary_decision,
                    item.shadow_decision,
                    item.input_hash
                )
            })
            .collect::<Vec<_>>()
            .join("");
        if rendered.is_empty() {
            "  (none)\n".to_string()
        } else {
            rendered
        }
    };
    format!(
        "Meridian Loom // SHADOW COMPARE\n================================\nprimary_log:     {}\nshadow_log:      {}\nprimary_events:  {}\nshadow_events:   {}\npairs_compared:  {}\nmatches:         {}\ndivergences:     {}\ndivergence_rate: {:.4}\n\nDivergence details\n------------------\n{}note:            {}\n",
        summary.primary_log.display(),
        summary.shadow_log.display(),
        summary.primary_events,
        summary.shadow_events,
        summary.pairs_compared,
        summary.matches,
        summary.divergences,
        summary.divergence_rate,
        divergence_lines,
        summary.note,
    )
}

pub fn render_compare_json(summary: &ComparisonSummary) -> String {
    let hook_results = summary
        .hook_results
        .iter()
        .map(|item| {
            format!(
                "    {{\"pair_index\":{},\"hook_name\":{},\"input_hash\":{},\"primary_decision\":{},\"shadow_decision\":{},\"matched\":{},\"primary_agent_id\":{},\"shadow_agent_id\":{},\"primary_org_id\":{},\"shadow_org_id\":{}}}",
                item.pair_index,
                json_string(&item.hook_name),
                json_string(&item.input_hash),
                json_string(&item.primary_decision),
                json_string(&item.shadow_decision),
                if item.matched { "true" } else { "false" },
                json_string(&item.primary_agent_id),
                json_string(&item.shadow_agent_id),
                json_string(&item.primary_org_id),
                json_string(&item.shadow_org_id),
            )
        })
        .collect::<Vec<_>>()
        .join(",\n");
    format!(
        "{{\n  \"status\": \"comparison_complete\",\n  \"primary_log\": {},\n  \"shadow_log\": {},\n  \"primary_events\": {},\n  \"shadow_events\": {},\n  \"pairs_compared\": {},\n  \"matches\": {},\n  \"divergences\": {},\n  \"divergence_rate\": {:.6},\n  \"hook_results\": [\n{}\n  ],\n  \"note\": {}\n}}\n",
        json_string(&summary.primary_log.display().to_string()),
        json_string(&summary.shadow_log.display().to_string()),
        summary.primary_events,
        summary.shadow_events,
        summary.pairs_compared,
        summary.matches,
        summary.divergences,
        summary.divergence_rate,
        hook_results,
        json_string(&summary.note),
    )
}

pub fn render_shadow_report(root: &Path) -> ShadowResult<String> {
    let report_path = root.join(".loom/shadow/latest.json");
    let contents = fs::read_to_string(&report_path).ok();
    let reference_path = root.join(".loom/shadow/reference.json");
    let reference = fs::read_to_string(&reference_path).ok();
    let decision_path = root.join(".loom/shadow/decision.json");
    let decision = fs::read_to_string(&decision_path).ok();
    let runtime_path = root.join(".loom/runtime/last_execution.json");
    let runtime = fs::read_to_string(&runtime_path).ok();
    let parity_path = root.join(".loom/parity/latest.json");
    let parity = fs::read_to_string(&parity_path).ok();
    if contents.is_none() && reference.is_none() && decision.is_none() && runtime.is_none() && parity.is_none() {
        return Err(format!(
            "could not read any shadow artifacts under {}",
            root.join(".loom/shadow").display()
        ));
    }
    let mut out = String::from(
        "Meridian Loom // SHADOW REPORT\n==============================\nphase:       experimental shadow + parity surface\nboundary:    report artifacts are real; governed runtime is not\n",
    );
    let stale_latest = contents
        .as_ref()
        .map(|value| value.contains("\"status\": \"not_started\""))
        .unwrap_or(false);
    let no_newer_artifacts =
        runtime.is_none() && parity.is_none() && decision.is_none() && reference.is_none();

    if stale_latest && no_newer_artifacts {
        out.push_str(&format!(
            "\nCurrent state\n=============\nsource: {}\nstatus:      not_started\nmeaning:     no shadow or runtime rehearsal artifacts have been captured yet\n\nRecommended next step\n=====================\n  loom shadow preflight --agent-id agent_atlas --action-type research --resource web_search --kernel-path /tmp/meridian-kernel --root {}\n  loom shadow decide --agent-id agent_atlas --action-type research --resource web_search --kernel-path /tmp/meridian-kernel --root {}\n  loom action execute --agent-id agent_atlas --action-type research --resource web_search --kernel-path /tmp/meridian-kernel --root {}\n",
            report_path.display(),
            root.display(),
            root.display(),
            root.display(),
        ));
        return Ok(out);
    }

    if let Some(runtime) = runtime.as_ref() {
        out.push_str(&format!(
            "Runtime execution\n=================\nsource: {}\n\n{}\n",
            runtime_path.display(),
            runtime
        ));
    }
    if let Some(parity) = parity.as_ref() {
        out.push_str(&format!(
            "\nParity latest\n=============\nsource: {}\n\n{}\n",
            parity_path.display(),
            parity
        ));
    }
    if let Some(decision) = decision.as_ref() {
        out.push_str(&format!(
            "\nDecision artifact\n=================\nsource: {}\n\n{}\n",
            decision_path.display(),
            decision
        ));
    }
    if let Some(reference) = reference.as_ref() {
        out.push_str(&format!(
            "\nReference gates\n===============\nsource: {}\n\n{}\n",
            reference_path.display(),
            reference
        ));
    }
    if let Some(contents) = contents.as_ref() {
        let label = if stale_latest && (runtime.is_some() || parity.is_some()) {
            "Legacy shadow marker"
        } else {
            "Shadow latest"
        };
        out.push_str(&format!(
            "\n{}\n{}\nsource: {}\n\n{}\n",
            label,
            "=".repeat(label.len()),
            report_path.display(),
            contents
        ));
    } else {
        out.push_str("\nShadow latest\n=============\nsource: (latest report not present)\n\n");
    }
    if stale_latest && runtime.is_some() {
        out.push_str(
            "Note\n----\nlatest shadow marker still says `not_started`; the runtime execution and parity sections above are the newer operator surfaces for this flow.\n",
        );
    }
    Ok(out)
}

pub fn render_parity_report(root: &Path) -> ShadowResult<String> {
    let report_path = root.join(".loom/parity/latest.json");
    let contents = fs::read_to_string(&report_path).ok();
    let stream_path = root.join(".loom/parity/stream.jsonl");
    let stream = fs::read_to_string(&stream_path).ok();
    let openclaw_live_path = root.join(".loom/parity/openclaw_live.json");
    let openclaw_live = fs::read_to_string(&openclaw_live_path).ok();
    let openclaw_stream_path = root.join(".loom/parity/openclaw_live_stream.jsonl");
    let openclaw_stream = fs::read_to_string(&openclaw_stream_path).ok();
    if contents.is_none() && stream.is_none() && openclaw_live.is_none() && openclaw_stream.is_none() {
        return Ok(format!(
            "Meridian Loom // PARITY REPORT\n===============================\nphase:       runtime-side parity surface\nboundary:    parity artifacts appear only after runtime rehearsal\n\nCurrent state\n=============\nstatus:      not_started\nmeaning:     no parity stream, parity report, or live OpenClaw probe has been captured yet\n\nRecommended next step\n=====================\n1. loom action execute --agent-id agent_atlas --action-type research --resource web_search --kernel-path /tmp/meridian-kernel --root {}\n2. loom shadow report --root {}\n3. Re-run loom parity report after runtime rehearsal artifacts exist.\n",
            root.display(),
            root.display(),
        ));
    }
    Ok(format!(
        "Meridian Loom // PARITY REPORT\n===============================\nphase:       runtime-side parity surface\nboundary:    parity report is real; per-action live runtime parity is still future work\n\nParity latest\n=============\nsource: {}\n\n{}\nParity stream\n=============\nsource: {}\n\n{}\n{}\n{}\n",
        report_path.display(),
        contents.unwrap_or_else(|| "{\n  \"status\": \"missing\",\n  \"note\": \"latest parity report has not been captured yet\"\n}\n".to_string()),
        stream_path.display(),
        stream.unwrap_or_else(|| "# parity stream not captured yet\n".to_string()),
        openclaw_live
            .map(|contents| format!(
                "OpenClaw live probe\n===================\nsource: {}\n\n{}\n",
                openclaw_live_path.display(),
                contents
            ))
            .unwrap_or_else(|| "OpenClaw live probe\n===================\nsource: (not captured)\n\n".to_string()),
        openclaw_stream
            .map(|contents| format!(
                "OpenClaw live probe stream\n==========================\nsource: {}\n\n{}\n",
                openclaw_stream_path.display(),
                contents
            ))
            .unwrap_or_else(|| "OpenClaw live probe stream\n==========================\nsource: (not captured)\n\n".to_string()),
    ))
}

fn render_parity_report_json(capture: &RuntimeExecutionCapture) -> String {
    format!(
        "{{\n  \"status\": \"parity_report_captured\",\n  \"execution_path\": {},\n  \"decision_path\": {},\n  \"audit_log_path\": {},\n  \"parity_stream_path\": {},\n  \"openclaw_live_probe_path\": {},\n  \"openclaw_live_probe_stream_path\": {},\n  \"reference_decision\": {},\n  \"reference_stage\": {},\n  \"overall_decision\": {},\n  \"effective_stage\": {},\n  \"openclaw_live_probe_status\": {},\n  \"openclaw_live_probe_note\": {},\n  \"parity_status\": {},\n  \"parity_reason\": {},\n  \"note\": \"runtime-side parity stream now captures Loom execution plus a per-action OpenClaw live probe artifact when available; hosted per-action parity remains future work\"\n}}\n",
        json_string(&capture.execution_path.display().to_string()),
        json_string(&capture.decision_path.display().to_string()),
        json_string(&capture.audit_log_path.display().to_string()),
        json_string(&capture.parity_stream_path.display().to_string()),
        capture
            .openclaw_live_probe_path
            .as_ref()
            .map(|path| json_string(&path.display().to_string()))
            .unwrap_or_else(|| "null".to_string()),
        capture
            .openclaw_live_probe_stream_path
            .as_ref()
            .map(|path| json_string(&path.display().to_string()))
            .unwrap_or_else(|| "null".to_string()),
        json_string(&capture.reference_decision),
        json_string(&capture.reference_stage),
        json_string(&capture.overall_decision),
        json_string(&capture.effective_stage),
        json_string(&capture.openclaw_live_probe_status),
        json_string(&capture.openclaw_live_probe_note),
        json_string(&capture.parity_status),
        json_string(&capture.parity_reason),
    )
}

fn ensure_shadow_dir(root: &Path) -> ShadowResult<PathBuf> {
    let shadow_dir = root.join(".loom/shadow");
    fs::create_dir_all(&shadow_dir).map_err(io_err)?;
    Ok(shadow_dir)
}

fn ensure_runtime_dir(root: &Path) -> ShadowResult<PathBuf> {
    let runtime_dir = root.join(".loom/runtime");
    fs::create_dir_all(&runtime_dir).map_err(io_err)?;
    Ok(runtime_dir)
}

fn ensure_runtime_jobs_dir(root: &Path) -> ShadowResult<PathBuf> {
    let jobs_dir = ensure_runtime_dir(root)?.join("jobs");
    fs::create_dir_all(&jobs_dir).map_err(io_err)?;
    Ok(jobs_dir)
}

fn ensure_audit_dir(root: &Path) -> ShadowResult<PathBuf> {
    let audit_dir = root.join(".loom/audit");
    fs::create_dir_all(&audit_dir).map_err(io_err)?;
    Ok(audit_dir)
}

fn runtime_audit_log_path(root: &Path, override_kernel_path: Option<&str>) -> PathBuf {
    match kernel_path_for(root, override_kernel_path) {
        Ok(kernel_path) => kernel_path
            .join("kernel")
            .join("runtime_audit")
            .join("loom_runtime_events.jsonl"),
        Err(_) => root.join(".loom/audit/runtime_events.jsonl"),
    }
}

fn write_supervisor_runtime_state(
    runtime_state_path: &Path,
    session_id: &str,
    pid: u32,
    running: bool,
    status: &str,
    booted_at: &str,
    stopped_at: &str,
    poll_seconds: u64,
    max_jobs: usize,
    max_iterations: usize,
    iterations_completed: usize,
    processed: usize,
    allowed: usize,
    denied: usize,
    failed: usize,
    pending_jobs: usize,
    processed_jobs: usize,
    failed_jobs: usize,
    note: String,
) -> ShadowResult<()> {
    fs::write(
        runtime_state_path,
        format!(
            "{{\n  \"status\": {},\n  \"updated_at\": {},\n  \"session_id\": {},\n  \"pid\": {},\n  \"running\": {},\n  \"booted_at\": {},\n  \"stopped_at\": {},\n  \"poll_seconds\": {},\n  \"max_jobs\": {},\n  \"max_iterations\": {},\n  \"iterations_completed\": {},\n  \"processed\": {},\n  \"allowed\": {},\n  \"denied\": {},\n  \"failed\": {},\n  \"pending_jobs\": {},\n  \"processed_jobs\": {},\n  \"failed_jobs\": {},\n  \"note\": {}\n}}\n",
            json_string(status),
            json_string(&timestamp_now()),
            json_string(session_id),
            pid,
            if running { "true" } else { "false" },
            json_string(booted_at),
            json_string(stopped_at),
            poll_seconds,
            max_jobs,
            max_iterations,
            iterations_completed,
            processed,
            allowed,
            denied,
            failed,
            pending_jobs,
            processed_jobs,
            failed_jobs,
            json_string(&note),
        ),
    )
    .map_err(io_err)
}

fn count_runtime_queue_entries(root: &Path, bucket: &str) -> ShadowResult<usize> {
    let path = ensure_runtime_dir(root)?.join("queue").join(bucket);
    if !path.exists() {
        return Ok(0);
    }
    Ok(fs::read_dir(path)
        .map_err(io_err)?
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().is_file())
        .count())
}

fn ensure_parity_dir(root: &Path) -> ShadowResult<PathBuf> {
    let parity_dir = root.join(".loom/parity");
    fs::create_dir_all(&parity_dir).map_err(io_err)?;
    Ok(parity_dir)
}

fn supervisor_config(root: &Path) -> ShadowResult<Config> {
    if let Ok(config) = read_config(root) {
        return Ok(config);
    }
    Ok(Config {
        mode: "embedded".to_string(),
        kernel_path: String::new(),
        org_id: "local_foundry".to_string(),
        state_dir: ".loom".to_string(),
        python_path: "workers/python".to_string(),
        typescript_path: "workers/typescript".to_string(),
        wasm_dir: "workers/wasm".to_string(),
    })
}

fn run_worker_supervisor(
    root: &Path,
    envelope: &ActionEnvelope,
    decision: &DecisionCapture,
) -> ShadowResult<WorkerExecutionCapture> {
    let config = supervisor_config(root)?;
    let worker_entry = ensure_runtime_worker_scaffold(root, &config)?;
    let jobs_dir = ensure_runtime_dir(root)?
        .join("jobs")
        .join(&decision.input_hash);
    fs::create_dir_all(&jobs_dir).map_err(io_err)?;
    let worker_request_path = jobs_dir.join("request.json");
    let worker_result_path = jobs_dir.join("result.json");
    let worker_log_path = jobs_dir.join("worker.log");
    let worker_kind = "python_reference_worker".to_string();

    fs::write(
        &worker_request_path,
        format!(
            "{{\n  \"input_hash\": {},\n  \"envelope\": {{\n    \"agent_id\": {},\n    \"org_id\": {},\n    \"runtime_id\": {},\n    \"action_type\": {},\n    \"resource\": {},\n    \"estimated_cost_usd\": {:.6}\n  }},\n  \"decision\": {{\n    \"overall_decision\": {},\n    \"effective_source\": {},\n    \"effective_stage\": {},\n    \"reference_stage\": {}\n  }}\n}}\n",
            json_string(&decision.input_hash),
            json_string(&envelope.agent_id),
            json_string(&envelope.org_id),
            json_string(&envelope.runtime_id),
            json_string(&envelope.action_type),
            json_string(&envelope.resource),
            envelope.estimated_cost_usd,
            json_string(&decision.overall_decision),
            json_string(&decision.effective_source),
            json_string(&decision.effective_stage),
            json_string(&decision.reference_stage),
        ),
    )
    .map_err(io_err)?;

    if decision.overall_decision != "allow" {
        fs::write(
            &worker_log_path,
            format!(
                "worker_not_dispatched decision={} stage={} reason={}\n",
                decision.overall_decision, decision.effective_stage, decision.effective_reason
            ),
        )
        .map_err(io_err)?;
        return Ok(WorkerExecutionCapture {
            worker_request_path,
            worker_result_path,
            worker_log_path,
            worker_status: "not_dispatched".to_string(),
            worker_kind,
            worker_note: "effective decision denied; supervisor did not dispatch worker".to_string(),
            runtime_outcome: "denied".to_string(),
        });
    }

    let output = Command::new("python3")
        .arg(runtime_worker_entry(root, &config))
        .arg("--input")
        .arg(&worker_request_path)
        .arg("--output")
        .arg(&worker_result_path)
        .output()
        .map_err(io_err)?;

    let mut log_contents = String::new();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stdout.trim().is_empty() {
        log_contents.push_str("stdout:\n");
        log_contents.push_str(stdout.trim());
        log_contents.push('\n');
    }
    if !stderr.trim().is_empty() {
        log_contents.push_str("stderr:\n");
        log_contents.push_str(stderr.trim());
        log_contents.push('\n');
    }
    fs::write(&worker_log_path, log_contents).map_err(io_err)?;

    if output.status.success() && worker_result_path.exists() {
        Ok(WorkerExecutionCapture {
            worker_request_path,
            worker_result_path,
            worker_log_path,
            worker_status: "completed".to_string(),
            worker_kind,
            worker_note: format!("experimental supervisor dispatched {}", worker_entry.display()),
            runtime_outcome: "worker_executed".to_string(),
        })
    } else {
        Ok(WorkerExecutionCapture {
            worker_request_path,
            worker_result_path,
            worker_log_path,
            worker_status: "failed".to_string(),
            worker_kind,
            worker_note: if stderr.trim().is_empty() {
                "worker supervisor command failed".to_string()
            } else {
                stderr.trim().to_string()
            },
            runtime_outcome: "worker_failed".to_string(),
        })
    }
}

fn capture_openclaw_live_probe(
    root: &Path,
    input_hash: &str,
) -> ShadowResult<(Option<PathBuf>, Option<PathBuf>, String, String)> {
    let parity_dir = ensure_parity_dir(root)?;
    let live_dir = parity_dir.join("openclaw");
    fs::create_dir_all(&live_dir).map_err(io_err)?;
    let probe_path = live_dir.join(format!("{}.json", input_hash));
    let latest_probe_path = parity_dir.join("openclaw_live.json");
    let probe_stream_path = parity_dir.join("openclaw_live_stream.jsonl");
    let proof_script = std::env::var("MERIDIAN_OPENCLAW_PROOF_SCRIPT")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from("/root/.openclaw/workspace/company/meridian_platform/openclaw_runtime_proof.py")
        });
    if !proof_script.exists() {
        append_line(
            &probe_stream_path,
            &format!(
                "{{\"timestamp\":{},\"input_hash\":{},\"status\":\"not_available\",\"reason\":{},\"probe_path\":null}}\n",
                json_string(&timestamp_now()),
                json_string(input_hash),
                json_string(&format!("live OpenClaw proof script not found at {}", proof_script.display())),
            ),
        )?;
        return Ok((
            None,
            Some(probe_stream_path),
            "not_available".to_string(),
            format!("live OpenClaw proof script not found at {}", proof_script.display()),
        ));
    }

    let output = Command::new("python3")
        .arg(&proof_script)
        .arg("--json")
        .output()
        .map_err(io_err)?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        append_line(
            &probe_stream_path,
            &format!(
                "{{\"timestamp\":{},\"input_hash\":{},\"status\":\"probe_failed\",\"reason\":{},\"probe_path\":null}}\n",
                json_string(&timestamp_now()),
                json_string(input_hash),
                json_string(if stderr.is_empty() {
                    "openclaw live proof command failed"
                } else {
                    &stderr
                }),
            ),
        )?;
        return Ok((
            None,
            Some(probe_stream_path),
            "probe_failed".to_string(),
            if stderr.is_empty() {
                "openclaw live proof command failed".to_string()
            } else {
                stderr
            },
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    fs::write(&probe_path, format!("{}\n", stdout)).map_err(io_err)?;
    fs::write(&latest_probe_path, format!("{}\n", stdout)).map_err(io_err)?;
    let health_ok = extract_json_bool(&stdout, "\"health_ok\"").unwrap_or(false);
    let proof_level =
        extract_json_string(&stdout, "\"proof_level\"").unwrap_or_else(|| "unknown".to_string());
    let deployment_mode =
        extract_json_string(&stdout, "\"deployment_mode\"").unwrap_or_else(|| "unknown".to_string());
    let note = format!(
        "live OpenClaw probe {} with proof_level={} deployment_mode={}",
        if health_ok { "healthy" } else { "degraded" },
        proof_level,
        deployment_mode,
    );
    append_line(
        &probe_stream_path,
        &format!(
            "{{\"timestamp\":{},\"input_hash\":{},\"status\":{},\"proof_level\":{},\"deployment_mode\":{},\"probe_path\":{}}}\n",
            json_string(&timestamp_now()),
            json_string(input_hash),
            json_string(if health_ok { "ok" } else { "degraded" }),
            json_string(&proof_level),
            json_string(&deployment_mode),
            json_string(&probe_path.display().to_string()),
        ),
    )?;
    Ok((
        Some(probe_path),
        Some(probe_stream_path),
        if health_ok { "ok" } else { "degraded" }.to_string(),
        note,
    ))
}

fn emit_kernel_audit_preview(
    kernel_path: &Path,
    audit_preview_log: &Path,
    envelope: &ActionEnvelope,
    input_hash: &str,
) -> ShadowResult<String> {
    let kernel_dir = kernel_path.join("kernel");
    if kernel_dir.join("audit.py").exists() {
        let script = r#"import sys
kernel_dir = sys.argv[1]
org_id = sys.argv[2]
agent_id = sys.argv[3]
action = sys.argv[4]
resource = sys.argv[5]
input_hash = sys.argv[6]
estimated_cost = float(sys.argv[7])
session_id = sys.argv[8]
sys.path.insert(0, kernel_dir)
import audit
event_id = audit.log_event(
    org_id,
    agent_id,
    action,
    resource=resource,
    outcome='simulated_success',
    actor_type='agent',
    details={
        'source': 'loom_shadow_preflight',
        'input_hash': input_hash,
        'estimated_cost_usd': estimated_cost,
        'experimental': True,
    },
    policy_ref='experimental_preflight_preview',
    session_id=session_id or None,
)
print(event_id)
"#;
        let output = Command::new("python3")
            .arg("-c")
            .arg(script)
            .arg(&kernel_dir)
            .arg(&envelope.org_id)
            .arg(&envelope.agent_id)
            .arg(&envelope.action_type)
            .arg(&envelope.resource)
            .arg(input_hash)
            .arg(format!("{:.6}", envelope.estimated_cost_usd))
            .arg(&envelope.session_id)
            .env("MERIDIAN_AUDIT_FILE", audit_preview_log)
            .output()
            .map_err(io_err)?;
        if output.status.success() {
            return Ok("kernel_preview_written".to_string());
        }
    }

    append_line(
        audit_preview_log,
        &format!(
            "{{\"id\":{},\"timestamp\":{},\"org_id\":{},\"agent_id\":{},\"actor_type\":\"agent\",\"action\":{},\"resource\":{},\"outcome\":\"simulated_success\",\"details\":{{\"source\":\"loom_shadow_preflight\",\"input_hash\":{},\"estimated_cost_usd\":{:.6},\"experimental\":true}},\"policy_ref\":\"experimental_preflight_preview\"}}\n",
            json_string(&format!("preview_{}", &input_hash[..8])),
            json_string(&timestamp_now()),
            json_string(&envelope.org_id),
            json_string(&envelope.agent_id),
            json_string(&envelope.action_type),
            json_string(&envelope.resource),
            json_string(input_hash),
            envelope.estimated_cost_usd,
        ),
    )?;
    Ok("local_preview_written".to_string())
}

fn emit_runtime_audit(
    kernel_path: &Path,
    audit_log_path: &Path,
    envelope: &ActionEnvelope,
    decision: &DecisionCapture,
    outcome: &str,
    worker: &WorkerExecutionCapture,
) -> ShadowResult<String> {
    let kernel_dir = kernel_path.join("kernel");
    if let Some(parent) = audit_log_path.parent() {
        fs::create_dir_all(parent).map_err(io_err)?;
    }
    if kernel_dir.join("audit.py").exists() {
        let output = Command::new("python3")
            .arg(kernel_dir.join("audit.py"))
            .arg("log-runtime")
            .arg("--org_id")
            .arg(&envelope.org_id)
            .arg("--agent_id")
            .arg(&envelope.agent_id)
            .arg("--action")
            .arg(&envelope.action_type)
            .arg("--resource")
            .arg(&envelope.resource)
            .arg("--outcome")
            .arg(outcome)
            .arg("--input_hash")
            .arg(&decision.input_hash)
            .arg("--estimated_cost_usd")
            .arg(format!("{:.6}", envelope.estimated_cost_usd))
            .arg("--effective_source")
            .arg(&decision.effective_source)
            .arg("--effective_stage")
            .arg(&decision.effective_stage)
            .arg("--reference_stage")
            .arg(&decision.reference_stage)
            .arg("--runtime_outcome")
            .arg(outcome)
            .arg("--worker_status")
            .arg(&worker.worker_status)
            .arg("--worker_kind")
            .arg(&worker.worker_kind)
            .arg("--parity_status")
            .arg(if decision.overall_decision == "allow" || decision.overall_decision == "deny" {
                if decision.effective_source == "reference_gate" { "match" } else { "divergence" }
            } else {
                "unknown"
            })
            .arg("--session_id")
            .arg(&envelope.session_id)
            .output()
            .map_err(io_err)?;
        if output.status.success() {
            return Ok("kernel_cli_runtime_event_written".to_string());
        }
    }

    append_line(
        audit_log_path,
        &format!(
            "{{\"id\":{},\"timestamp\":{},\"org_id\":{},\"agent_id\":{},\"actor_type\":\"agent\",\"action\":{},\"resource\":{},\"outcome\":{},\"details\":{{\"source\":\"loom_runtime_execute\",\"input_hash\":{},\"estimated_cost_usd\":{:.6},\"effective_source\":{},\"effective_stage\":{},\"reference_stage\":{},\"runtime_outcome\":{},\"worker_status\":{},\"worker_kind\":{},\"experimental\":true}},\"policy_ref\":\"experimental_runtime_rehearsal\"}}\n",
            json_string(&format!("runtime_{}", &decision.input_hash[..8])),
            json_string(&timestamp_now()),
            json_string(&envelope.org_id),
            json_string(&envelope.agent_id),
            json_string(&envelope.action_type),
            json_string(&envelope.resource),
            json_string(outcome),
            json_string(&decision.input_hash),
            envelope.estimated_cost_usd,
            json_string(&decision.effective_source),
            json_string(&decision.effective_stage),
            json_string(&decision.reference_stage),
            json_string(outcome),
            json_string(&worker.worker_status),
            json_string(&worker.worker_kind),
        ),
    )?;
    Ok("runtime_event_written_local_fallback".to_string())
}

fn append_line(path: &Path, line: &str) -> ShadowResult<()> {
    let mut existing = if path.exists() {
        fs::read_to_string(path).map_err(io_err)?
    } else {
        String::new()
    };
    existing.push_str(line);
    fs::write(path, existing).map_err(io_err)?;
    Ok(())
}

fn job_snapshot_path(root: &Path, job_id: &str) -> PathBuf {
    root.join(".loom/runtime/jobs").join(job_id).join("job.json")
}

fn write_job_snapshot(root: &Path, snapshot: JobSnapshot) -> ShadowResult<()> {
    let jobs_dir = ensure_runtime_jobs_dir(root)?;
    let job_dir = jobs_dir.join(&snapshot.job_id);
    fs::create_dir_all(&job_dir).map_err(io_err)?;
    let job_path = job_dir.join("job.json");
    fs::write(&job_path, render_job_snapshot_json(&snapshot)).map_err(io_err)?;
    let ledger_path = jobs_dir.join("ledger.jsonl");
    append_line(
        &ledger_path,
        &format!(
            "{{\"timestamp\":{},\"job_id\":{},\"status\":{},\"stage\":{},\"queue_bucket\":{},\"agent_id\":{},\"org_id\":{},\"action_type\":{},\"resource\":{},\"updated_at\":{},\"job_path\":{}}}\n",
            json_string(&snapshot.updated_at),
            json_string(&snapshot.job_id),
            json_string(&snapshot.status),
            json_string(&snapshot.stage),
            json_string(&snapshot.queue_bucket),
            json_string(&snapshot.agent_id),
            json_string(&snapshot.org_id),
            json_string(&snapshot.action_type),
            json_string(&snapshot.resource),
            json_string(&snapshot.updated_at),
            json_string(&job_path.display().to_string()),
        ),
    )?;
    Ok(())
}

fn read_job_snapshot(root: &Path, job_id: &str) -> ShadowResult<JobSnapshot> {
    let job_path = job_snapshot_path(root, job_id);
    let contents = fs::read_to_string(&job_path).map_err(io_err)?;
    Ok(JobSnapshot {
        root: root.to_path_buf(),
        job_id: extract_json_string(&contents, "\"job_id\"")
            .unwrap_or_else(|| job_id.to_string()),
        job_path,
        status: extract_json_string(&contents, "\"job_status\"").unwrap_or_default(),
        stage: extract_json_string(&contents, "\"job_stage\"").unwrap_or_default(),
        queue_bucket: extract_json_string(&contents, "\"queue_bucket\"").unwrap_or_else(|| "(none)".to_string()),
        queued_at: extract_json_string(&contents, "\"queued_at\"").unwrap_or_default(),
        updated_at: extract_json_string(&contents, "\"updated_at\"").unwrap_or_default(),
        agent_id: extract_json_string(&contents, "\"agent_id\"").unwrap_or_default(),
        org_id: extract_json_string(&contents, "\"org_id\"").unwrap_or_default(),
        action_type: extract_json_string(&contents, "\"action_type\"").unwrap_or_default(),
        resource: extract_json_string(&contents, "\"resource\"").unwrap_or_default(),
        estimated_cost_usd: extract_json_string(&contents, "\"estimated_cost_usd\"")
            .unwrap_or_else(|| "0.000000".to_string()),
        runtime_outcome: extract_json_string(&contents, "\"runtime_outcome\"")
            .unwrap_or_else(|| "not_started".to_string()),
        worker_status: extract_json_string(&contents, "\"worker_status\"")
            .unwrap_or_else(|| "not_started".to_string()),
        queue_path: extract_optional_path(&contents, "\"queue_path\""),
        decision_path: extract_optional_path(&contents, "\"decision_path\""),
        execution_path: extract_optional_path(&contents, "\"execution_path\""),
        audit_log_path: extract_optional_path(&contents, "\"audit_log_path\""),
        parity_report_path: extract_optional_path(&contents, "\"parity_report_path\""),
        note: extract_json_string(&contents, "\"note\"").unwrap_or_default(),
    })
}

fn sanitize_filename(input: &str) -> String {
    input
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect()
}

fn load_events(path: &Path) -> ShadowResult<Vec<ShadowEvent>> {
    let contents = fs::read_to_string(path).map_err(io_err)?;
    let mut events = Vec::new();
    for line in contents.lines() {
        let raw = line.trim();
        if raw.is_empty() {
            continue;
        }
        events.push(ShadowEvent {
            hook_name: extract_json_string(raw, "\"hook_name\"")
                .ok_or_else(|| format!("hook_name missing in {}", path.display()))?,
            input_hash: extract_json_string(raw, "\"input_hash\"")
                .ok_or_else(|| format!("input_hash missing in {}", path.display()))?,
            decision: extract_json_string(raw, "\"decision\"")
                .ok_or_else(|| format!("decision missing in {}", path.display()))?,
            agent_id: extract_json_string(raw, "\"agent_id\"")
                .ok_or_else(|| format!("agent_id missing in {}", path.display()))?,
            org_id: extract_json_string(raw, "\"org_id\"")
                .ok_or_else(|| format!("org_id missing in {}", path.display()))?,
        });
    }
    Ok(events)
}

fn extract_json_string(section: &str, key: &str) -> Option<String> {
    let idx = section.find(key)?;
    let after = &section[idx + key.len()..];
    let first_quote = after.find('"')?;
    let rest = &after[first_quote + 1..];
    let end_quote = rest.find('"')?;
    Some(rest[..end_quote].to_string())
}

fn extract_json_bool(section: &str, key: &str) -> Option<bool> {
    let idx = section.find(key)?;
    let after = &section[idx + key.len()..];
    if after.contains("true") {
        Some(true)
    } else if after.contains("false") {
        Some(false)
    } else {
        None
    }
}

fn extract_json_number(section: &str, key: &str) -> Option<f64> {
    let idx = section.find(key)?;
    let after = &section[idx + key.len()..];
    let colon = after.find(':')?;
    let rest = after[colon + 1..].trim_start();
    let end = rest
        .find(|c: char| c == ',' || c == '\n' || c == '}')
        .unwrap_or(rest.len());
    rest[..end].trim().parse::<f64>().ok()
}

fn extract_optional_path(section: &str, key: &str) -> Option<PathBuf> {
    extract_json_string(section, key)
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
}

fn json_string(input: &str) -> String {
    format!("{:?}", input)
}

fn render_json_string_array(values: &[String]) -> String {
    let items = values
        .iter()
        .map(|value| json_string(value))
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{}]", items)
}

fn io_err(error: impl std::fmt::Display) -> String {
    error.to_string()
}

fn display_optional_path(path: Option<&PathBuf>) -> String {
    path.map(|value| value.display().to_string())
        .unwrap_or_else(|| "(not captured)".to_string())
}

fn render_job_snapshot_json(snapshot: &JobSnapshot) -> String {
    format!(
        "    {{\n      \"job_id\": {},\n      \"job_path\": {},\n      \"job_status\": {},\n      \"job_stage\": {},\n      \"queue_bucket\": {},\n      \"queued_at\": {},\n      \"updated_at\": {},\n      \"agent_id\": {},\n      \"org_id\": {},\n      \"action_type\": {},\n      \"resource\": {},\n      \"estimated_cost_usd\": {},\n      \"runtime_outcome\": {},\n      \"worker_status\": {},\n      \"queue_path\": {},\n      \"decision_path\": {},\n      \"execution_path\": {},\n      \"audit_log_path\": {},\n      \"parity_report_path\": {},\n      \"note\": {}\n    }}",
        json_string(&snapshot.job_id),
        json_string(&snapshot.job_path.display().to_string()),
        json_string(&snapshot.status),
        json_string(&snapshot.stage),
        json_string(&snapshot.queue_bucket),
        json_string(&snapshot.queued_at),
        json_string(&snapshot.updated_at),
        json_string(&snapshot.agent_id),
        json_string(&snapshot.org_id),
        json_string(&snapshot.action_type),
        json_string(&snapshot.resource),
        json_string(&snapshot.estimated_cost_usd),
        json_string(&snapshot.runtime_outcome),
        json_string(&snapshot.worker_status),
        snapshot
            .queue_path
            .as_ref()
            .map(|path| json_string(&path.display().to_string()))
            .unwrap_or_else(|| "null".to_string()),
        snapshot
            .decision_path
            .as_ref()
            .map(|path| json_string(&path.display().to_string()))
            .unwrap_or_else(|| "null".to_string()),
        snapshot
            .execution_path
            .as_ref()
            .map(|path| json_string(&path.display().to_string()))
            .unwrap_or_else(|| "null".to_string()),
        snapshot
            .audit_log_path
            .as_ref()
            .map(|path| json_string(&path.display().to_string()))
            .unwrap_or_else(|| "null".to_string()),
        snapshot
            .parity_report_path
            .as_ref()
            .map(|path| json_string(&path.display().to_string()))
            .unwrap_or_else(|| "null".to_string()),
        json_string(&snapshot.note),
    )
}

fn timestamp_now() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use loom_core::{init_workspace, ActionEnvelope, AgentIdentityResolution};

    #[test]
    fn records_preflight_and_renders_report() {
        let root = temp_path("loom-shadow-capture");
        fs::create_dir_all(root.join(".loom/shadow")).expect("shadow dir");
        let identity = sample_identity();
        let envelope = sample_envelope();
        let reference = ReferenceGateCheck {
            allowed: true,
            stage: "ok".to_string(),
            reason: "ok".to_string(),
            restrictions: vec![],
            sanction_gate_decision: "allow".to_string(),
            approval_gate_decision: "allow".to_string(),
            budget_gate_decision: "allow".to_string(),
            source: "kernel_reference_adapter_read_only".to_string(),
        };

        let capture =
            capture_preflight(&root, &root, &identity, &envelope, &reference).expect("capture");
        assert!(capture.event_log.exists());
        assert!(capture.audit_preview_log.exists());
        assert!(capture.reference_report.exists());
        assert!(capture.reference_event_log.exists());
        assert_eq!(capture.sanction_decision, "clear");
        assert_eq!(capture.sanction_gate_decision, "allow");
        assert_eq!(capture.identity_restrictions, Vec::<String>::new());
        assert_eq!(capture.reference_restrictions, Vec::<String>::new());
        assert_eq!(capture.budget_gate_decision, "allow");
        assert_eq!(capture.approval_decision, "not_required");
        assert_eq!(capture.approval_gate_decision, "allow");
        assert_eq!(capture.audit_emission_decision, "local_preview_written");
        assert_eq!(capture.overall_decision, "allow");
        let report = render_shadow_report(&root).expect("report");
        assert!(report.contains("preflight_captured"));
        assert!(report.contains("budget_gate"));
        assert!(report.contains("audit_preview_log"));
        assert!(report.contains("reference_report"));
    }

    #[test]
    fn decision_capture_surfaces_gate_outcome() {
        let root = temp_path("loom-shadow-decision");
        fs::create_dir_all(root.join(".loom/shadow")).expect("shadow dir");
        let identity = sample_identity();
        let envelope = sample_envelope();
        let reference = ReferenceGateCheck {
            allowed: false,
            stage: "budget_gate".to_string(),
            reason: "runway below reserve floor".to_string(),
            restrictions: vec!["execute".to_string()],
            sanction_gate_decision: "allow".to_string(),
            approval_gate_decision: "allow".to_string(),
            budget_gate_decision: "deny".to_string(),
            source: "kernel_reference_adapter_read_only".to_string(),
        };

        let capture = capture_decision(&root, &identity, &envelope, &reference).expect("decision");
        assert!(capture.decision_path.exists());
        assert_eq!(capture.overall_decision, "deny");
        assert!(capture.identity_restrictions.is_empty());
        assert_eq!(capture.reference_restrictions, vec!["execute".to_string()]);
        assert!(capture.local_sanction_allowed);
        assert_eq!(capture.local_sanction_decision, "allow");
        assert_eq!(capture.effective_source, "reference_gate");
        assert_eq!(capture.effective_stage, "budget_gate");
        assert_eq!(capture.reference_stage, "budget_gate");
        let human = render_decision_human(&capture);
        assert!(human.contains("overall_decision:       deny"));
        assert!(human.contains("identity_restrictions:  (none)"));
        assert!(human.contains("reference_restrictions: execute"));
        assert!(human.contains("effective_source:       reference_gate"));
        assert!(human.contains("reference_stage:        budget_gate"));
        let json = render_decision_json(&capture);
        assert!(json.contains("\"status\": \"decision_captured\""));
        assert!(json.contains("\"identity_restrictions\": []"));
        assert!(json.contains("\"reference_restrictions\": [\"execute\"]"));
        assert!(json.contains("\"overall_decision\": \"deny\""));
        assert!(json.contains("\"note\": \"experimental preflight decision only; not governed runtime enforcement\""));
        assert_eq!(decision_exit_code(&capture, 0, 2), 2);
        let report = render_shadow_report(&root).expect("render report");
        assert!(report.contains("Decision artifact"));
        assert!(report.contains("\"overall_decision\": \"deny\""));
    }

    #[test]
    fn decision_capture_prefers_local_sanction_preview_when_execute_is_restricted() {
        let root = temp_path("loom-shadow-decision-sanction");
        fs::create_dir_all(root.join(".loom/shadow")).expect("shadow dir");
        let mut identity = sample_identity();
        identity.restrictions = vec!["execute".to_string()];
        identity.sanction_decision = "restricted_execute".to_string();
        let envelope = sample_envelope();
        let reference = ReferenceGateCheck {
            allowed: true,
            stage: "ok".to_string(),
            reason: "ok".to_string(),
            restrictions: vec!["execute".to_string()],
            sanction_gate_decision: "allow".to_string(),
            approval_gate_decision: "allow".to_string(),
            budget_gate_decision: "allow".to_string(),
            source: "kernel_reference_adapter_read_only".to_string(),
        };

        let capture = capture_decision(&root, &identity, &envelope, &reference).expect("decision");
        assert_eq!(capture.overall_decision, "deny");
        assert_eq!(capture.identity_restrictions, vec!["execute".to_string()]);
        assert_eq!(capture.reference_restrictions, vec!["execute".to_string()]);
        assert!(!capture.local_sanction_allowed);
        assert_eq!(capture.local_sanction_decision, "deny");
        assert_eq!(capture.effective_source, "local_sanction_preview");
        assert_eq!(capture.effective_stage, "sanction_controls_preview");
        assert!(capture.effective_reason.contains("restricted from execute"));
        assert_eq!(decision_exit_code(&capture, 0, 2), 2);
    }

    #[test]
    fn compares_identical_logs_without_divergence() {
        let root = temp_path("loom-shadow-compare");
        let shadow_dir = root.join(".loom/shadow");
        fs::create_dir_all(&shadow_dir).expect("shadow dir");
        let log_a = shadow_dir.join("primary.jsonl");
        let log_b = shadow_dir.join("shadow.jsonl");
        let line = "{\"hook_name\":\"agent_identity\",\"input_hash\":\"abc\",\"decision\":\"resolved\",\"agent_id\":\"agent_atlas\",\"org_id\":\"org_demo\"}\n";
        fs::write(&log_a, line).expect("write primary");
        fs::write(&log_b, line).expect("write shadow");

        let summary = compare_logs(Some(&root), &log_a, &log_b).expect("compare");
        assert_eq!(summary.matches, 1);
        assert_eq!(summary.divergences, 0);
        assert_eq!(summary.hook_results.len(), 1);
        assert!(summary.hook_results[0].matched);
        let report = render_shadow_report(&root).expect("render report");
        assert!(report.contains("comparison_complete"));
        assert!(report.contains("\"hook_results\""));
    }

    #[test]
    fn compare_report_surfaces_hook_level_divergence_details() {
        let root = temp_path("loom-shadow-divergence");
        let shadow_dir = root.join(".loom/shadow");
        fs::create_dir_all(&shadow_dir).expect("shadow dir");
        let primary = shadow_dir.join("primary.jsonl");
        let shadow = shadow_dir.join("shadow.jsonl");
        fs::write(
            &primary,
            "{\"hook_name\":\"audit_emission\",\"input_hash\":\"abc\",\"decision\":\"not_exercised\",\"agent_id\":\"agent_atlas\",\"org_id\":\"org_demo\"}\n",
        )
        .expect("write primary");
        fs::write(
            &shadow,
            "{\"hook_name\":\"audit_emission\",\"input_hash\":\"abc\",\"decision\":\"kernel_preview_written\",\"agent_id\":\"agent_atlas\",\"org_id\":\"org_demo\"}\n",
        )
        .expect("write shadow");

        let summary = compare_logs(Some(&root), &primary, &shadow).expect("compare");
        assert_eq!(summary.matches, 0);
        assert_eq!(summary.divergences, 1);
        assert_eq!(summary.hook_results.len(), 1);
        assert_eq!(summary.hook_results[0].hook_name, "audit_emission");
        assert!(!summary.hook_results[0].matched);
        let human = render_compare_human(&summary);
        assert!(human.contains("Divergence details"));
        assert!(human.contains("[0] audit_emission | primary=not_exercised | shadow=kernel_preview_written | input=abc"));
        let json = render_compare_json(&summary);
        assert!(json.contains("\"hook_results\""));
        assert!(json.contains("\"shadow_decision\":\"kernel_preview_written\""));
    }

    #[test]
    fn runtime_execution_capture_writes_runtime_and_parity_artifacts() {
        let root = temp_path("loom-shadow-runtime");
        fs::create_dir_all(root.join(".loom/shadow")).expect("shadow dir");
        let identity = sample_identity();
        let envelope = sample_envelope();
        let reference = ReferenceGateCheck {
            allowed: true,
            stage: "ok".to_string(),
            reason: "ok".to_string(),
            restrictions: vec![],
            sanction_gate_decision: "allow".to_string(),
            approval_gate_decision: "allow".to_string(),
            budget_gate_decision: "allow".to_string(),
            source: "kernel_reference_adapter_read_only".to_string(),
        };

        let decision = capture_decision(&root, &identity, &envelope, &reference).expect("decision");
        let capture =
            capture_runtime_execution(&root, &root, &envelope, &reference, &decision).expect("runtime capture");

        assert!(capture.execution_path.exists());
        assert!(capture.worker_request_path.exists());
        assert!(capture.worker_result_path.exists());
        assert!(capture.worker_log_path.exists());
        assert!(capture.audit_log_path.exists());
        assert!(capture.parity_stream_path.exists());
        assert!(capture.parity_report_path.exists());
        assert_eq!(capture.runtime_outcome, "worker_executed");
        assert_eq!(capture.worker_status, "completed");
        assert_eq!(capture.reference_decision, "allow");
        assert_eq!(capture.parity_status, "match");

        let human = render_runtime_execution_human(&capture);
        assert!(human.contains("Meridian Loom // RUNTIME EXECUTE"));
        assert!(human.contains("phase:       experimental runtime rehearsal"));
        assert!(human.contains("worker supervisor artifacts"));
        assert!(human.contains("audit / parity artifacts"));
        assert!(human.contains("loom parity report --root"));
        let json = render_runtime_execution_json(&capture);
        assert!(json.contains("\"status\": \"runtime_execution_captured\""));
        assert!(json.contains("\"worker_status\": \"completed\""));
        assert!(json.contains("\"openclaw_live_probe_status\":"));
        let parity = render_parity_report(&root).expect("parity report");
        assert!(parity.contains("Meridian Loom // PARITY REPORT"));
        assert!(parity.contains("OpenClaw live probe stream"));
        assert!(parity.contains("\"parity_status\": \"match\""));
    }

    #[test]
    fn shadow_report_deprioritizes_stale_not_started_marker_when_runtime_exists() {
        let root = temp_path("loom-shadow-stale-latest");
        fs::create_dir_all(root.join(".loom/shadow")).expect("shadow dir");
        fs::create_dir_all(root.join(".loom/runtime")).expect("runtime dir");
        fs::create_dir_all(root.join(".loom/parity")).expect("parity dir");
        fs::write(
            root.join(".loom/shadow/latest.json"),
            "{\n  \"status\": \"not_started\",\n  \"note\": \"shadow mode is not implemented in this scaffold\"\n}\n",
        )
        .expect("write latest");
        fs::write(
            root.join(".loom/runtime/last_execution.json"),
            "{\n  \"status\": \"runtime_execution_captured\"\n}\n",
        )
        .expect("write runtime");
        fs::write(
            root.join(".loom/parity/latest.json"),
            "{\n  \"status\": \"parity_report_captured\"\n}\n",
        )
        .expect("write parity");

        let report = render_shadow_report(&root).expect("render report");
        let runtime_index = report.find("Runtime execution").expect("runtime section");
        let legacy_index = report.find("Legacy shadow marker").expect("legacy section");
        assert!(runtime_index < legacy_index);
        assert!(report.contains("the runtime execution and parity sections above are the newer operator surfaces"));
    }

    #[test]
    fn shadow_report_guides_next_steps_when_only_legacy_marker_exists() {
        let root = temp_path("loom-shadow-guidance");
        fs::create_dir_all(root.join(".loom/shadow")).expect("shadow dir");
        fs::write(
            root.join(".loom/shadow/latest.json"),
            "{\n  \"status\": \"not_started\",\n  \"note\": \"shadow mode is not implemented in this scaffold\"\n}\n",
        )
        .expect("write latest");

        let report = render_shadow_report(&root).expect("render report");
        assert!(report.contains("Current state"));
        assert!(report.contains("Recommended next step"));
        assert!(report.contains("loom shadow preflight"));
        assert!(report.contains("meaning:     no shadow or runtime rehearsal artifacts have been captured yet"));
    }

    #[test]
    fn parity_report_guides_next_steps_when_no_artifacts_exist() {
        let root = temp_path("loom-parity-guidance");
        fs::create_dir_all(root.join(".loom/parity")).expect("parity dir");

        let report = render_parity_report(&root).expect("render parity");
        assert!(report.contains("Current state"));
        assert!(report.contains("status:      not_started"));
        assert!(report.contains("loom action execute"));
        assert!(report.contains("loom shadow report"));
    }

    #[test]
    fn enqueue_action_writes_pending_queue_artifact() {
        let root = temp_path("loom-shadow-enqueue");
        fs::create_dir_all(&root).expect("root");
        let envelope = sample_envelope();

        let capture = enqueue_action(&root, Path::new("/tmp/meridian-kernel"), &envelope)
            .expect("enqueue");
        assert!(capture.queue_path.exists());
        assert!(capture.job_path.exists());
        let queued = fs::read_to_string(&capture.queue_path).expect("queued file");
        assert!(queued.contains("\"status\": \"queued\""));
        assert!(queued.contains("\"agent_id\": \"agent_atlas\""));
        assert!(queued.contains("\"kernel_path\": \"/tmp/meridian-kernel\""));
        let job = fs::read_to_string(&capture.job_path).expect("job file");
        assert!(job.contains("\"job_status\": \"queued\""));
        assert!(job.contains("\"queue_bucket\": \"pending\""));
    }

    #[test]
    fn job_list_and_inspect_surface_runtime_owned_state() {
        let root = temp_path("loom-shadow-job-ledger");
        fs::create_dir_all(&root).expect("root");
        let kernel_root = temp_path("loom-shadow-job-ledger-kernel");
        let kernel_dir = kernel_root.join("kernel");
        fs::create_dir_all(kernel_dir.join("adapters")).expect("kernel dirs");
        fs::write(
            kernel_dir.join("runtimes.json"),
            "{\n  \"runtimes\": {\n    \"local_kernel\": {\"id\": \"local_kernel\", \"label\": \"Local Kernel Runtime\"},\n    \"meridian_loom\": {\"status\": \"experimental\", \"notes\": \"job fixture\", \"contract_compliance\": {\"agent_identity\": null, \"action_envelope\": null, \"cost_attribution\": null, \"approval_hook\": null, \"audit_emission\": null, \"sanction_controls\": null, \"budget_gate\": null}}\n  }\n}\n",
        )
        .expect("write runtimes");
        fs::write(
            kernel_dir.join("agent_registry.py"),
            "import json, sys\nagent_id = sys.argv[sys.argv.index('--agent_id') + 1]\norg_id = sys.argv[sys.argv.index('--org_id') + 1] if '--org_id' in sys.argv else 'org_demo'\nprint(json.dumps({'id': agent_id, 'name': 'Atlas', 'org_id': org_id, 'role': 'analyst', 'economy_key': 'atlas', 'approval_required': False, 'budget': {'max_per_run_usd': 0.5}, 'runtime_binding': {'runtime_id': 'local_kernel', 'runtime_label': 'Local Kernel Runtime', 'bound_org_id': org_id, 'boundary_name': 'workspace', 'identity_model': 'session', 'runtime_registered': True, 'registration_status': 'registered'}}, indent=2))\n",
        )
        .expect("write registry");
        fs::write(kernel_dir.join("court.py"), "def get_restrictions(agent_id, org_id=None):\n    return []\n")
            .expect("write court");
        fs::write(kernel_dir.join("authority.py"), "def check_authority(agent_id, action, org_id=None):\n    return True, 'ok'\n")
            .expect("write authority");
        fs::write(kernel_dir.join("treasury.py"), "def check_budget(agent_id, cost_usd, org_id=None):\n    return True, 'ok'\n")
            .expect("write treasury");
        fs::write(kernel_dir.join("audit.py"), "def log_event(*args, **kwargs):\n    return 'evt_jobs'\n")
            .expect("write audit");
        fs::write(kernel_dir.join("adapters/__init__.py"), "").expect("write adapter init");
        fs::write(
            kernel_dir.join("adapters/openclaw_compatible.py"),
            "def pre_action_check(org_id, envelope):\n    return {'allowed': True, 'stage': 'ok', 'reason': 'ok', 'restrictions': []}\n",
        )
        .expect("write adapter");
        init_workspace(
            &root,
            "embedded",
            Some(kernel_root.to_string_lossy().as_ref()),
            "org_demo",
        )
        .expect("init workspace");
        let envelope = sample_envelope();
        let capture = enqueue_action(&root, &kernel_root, &envelope).expect("enqueue");
        let queued = inspect_job(&root, &capture.input_hash).expect("queued job");
        assert_eq!(queued.status, "queued");
        assert_eq!(queued.queue_bucket, "pending");
        let queued_human = render_job_inspect_human(&queued);
        assert!(queued_human.contains("JOB INSPECT"));

        run_supervisor(&root, Some(kernel_root.to_string_lossy().as_ref()), 1).expect("supervisor");
        let jobs = list_jobs(&root, None, 10).expect("job list");
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].status, "completed");
        assert_eq!(jobs[0].queue_bucket, "processed");
        assert!(jobs[0].execution_path.is_some());
        let list_human = render_job_list_human(&root, &jobs, None);
        assert!(list_human.contains("experimental runtime-owned job ledger"));
        assert!(list_human.contains("status=completed"));
    }

    #[test]
    fn supervisor_run_processes_pending_queue_and_uses_kernel_runtime_audit_path() {
        let root = temp_path("loom-shadow-supervisor");
        fs::create_dir_all(&root).expect("root");
        let kernel_root = temp_path("loom-shadow-supervisor-kernel");
        let kernel_dir = kernel_root.join("kernel");
        fs::create_dir_all(&kernel_dir).expect("kernel dir");
        fs::write(
            kernel_dir.join("runtimes.json"),
            "{\n  \"runtimes\": {\n    \"local_kernel\": {\"id\": \"local_kernel\", \"label\": \"Local Kernel Runtime\"},\n    \"meridian_loom\": {\"status\": \"experimental\", \"notes\": \"test note\", \"contract_compliance\": {\"agent_identity\": null, \"action_envelope\": null, \"cost_attribution\": null, \"approval_hook\": null, \"audit_emission\": null, \"sanction_controls\": null, \"budget_gate\": null}}\n  }\n}\n",
        )
        .expect("write runtimes");
        fs::write(
            kernel_dir.join("agent_registry.py"),
            "import json, sys\nagent_id = sys.argv[sys.argv.index('--agent_id') + 1]\norg_id = sys.argv[sys.argv.index('--org_id') + 1] if '--org_id' in sys.argv else 'org_demo'\nprint(json.dumps({'id': agent_id, 'name': 'Atlas', 'org_id': org_id, 'role': 'analyst', 'economy_key': 'atlas', 'approval_required': False, 'budget': {'max_per_run_usd': 0.5}, 'runtime_binding': {'runtime_id': 'local_kernel', 'runtime_label': 'Local Kernel Runtime', 'bound_org_id': org_id, 'boundary_name': 'workspace', 'identity_model': 'session', 'runtime_registered': True, 'registration_status': 'registered'}}, indent=2))\n",
        )
        .expect("write agent registry");
        fs::write(
            kernel_dir.join("court.py"),
            "def get_restrictions(agent_id, org_id=None):\n    return []\n",
        )
        .expect("write court");
        fs::write(
            kernel_dir.join("authority.py"),
            "def check_authority(agent_id, action, org_id=None):\n    return True, 'ok'\n",
        )
        .expect("write authority");
        fs::write(
            kernel_dir.join("treasury.py"),
            "def check_budget(agent_id, cost_usd, org_id=None):\n    if cost_usd > 0.5:\n        return False, 'below reserve'\n    return True, 'ok'\n",
        )
        .expect("write treasury");
        let adapters_dir = kernel_dir.join("adapters");
        fs::create_dir_all(&adapters_dir).expect("adapters dir");
        fs::write(adapters_dir.join("__init__.py"), "").expect("write adapters init");
        fs::write(
            adapters_dir.join("openclaw_compatible.py"),
            "import treasury\n\ndef pre_action_check(org_id, envelope):\n    allowed, reason = treasury.check_budget(envelope.get('agent_id'), float(envelope.get('estimated_cost_usd', 0.0)), org_id)\n    if not allowed:\n        return {'allowed': False, 'stage': 'budget_gate', 'reason': reason, 'restrictions': []}\n    return {'allowed': True, 'stage': 'ok', 'reason': 'ok', 'restrictions': []}\n",
        )
        .expect("write adapter");
        fs::write(
            kernel_dir.join("audit.py"),
            r#"#!/usr/bin/env python3
import argparse
import datetime
import json
import os
import uuid

PLATFORM_DIR = os.path.dirname(os.path.abspath(__file__))
RUNTIME_AUDIT_FILE = os.path.join(PLATFORM_DIR, "runtime_audit", "loom_runtime_events.jsonl")

def _now():
    return datetime.datetime.utcnow().strftime('%Y-%m-%dT%H:%M:%SZ')

def log_event(org_id, agent_id, action, resource='', outcome='success', details=None, policy_ref='', session_id=None, audit_file=None):
    audit_file = audit_file or RUNTIME_AUDIT_FILE
    os.makedirs(os.path.dirname(audit_file), exist_ok=True)
    event = {
        'id': f'evt_{uuid.uuid4().hex[:10]}',
        'timestamp': _now(),
        'org_id': org_id,
        'agent_id': agent_id,
        'action': action,
        'resource': resource,
        'outcome': outcome,
        'details': details or {},
        'policy_ref': policy_ref,
    }
    if session_id:
        event['session_id'] = session_id
    with open(audit_file, 'a', encoding='utf-8') as handle:
        handle.write(json.dumps(event) + '\n')

def main():
    parser = argparse.ArgumentParser()
    sub = parser.add_subparsers(dest='command')
    runtime = sub.add_parser('log-runtime')
    runtime.add_argument('--org_id', required=True)
    runtime.add_argument('--agent_id', default='')
    runtime.add_argument('--action', required=True)
    runtime.add_argument('--resource', default='')
    runtime.add_argument('--outcome', required=True)
    runtime.add_argument('--input_hash', required=True)
    runtime.add_argument('--estimated_cost_usd', type=float, required=True)
    runtime.add_argument('--effective_source', required=True)
    runtime.add_argument('--effective_stage', required=True)
    runtime.add_argument('--reference_stage', required=True)
    runtime.add_argument('--runtime_outcome', required=True)
    runtime.add_argument('--worker_status', default='')
    runtime.add_argument('--worker_kind', default='')
    runtime.add_argument('--parity_status', default='')
    runtime.add_argument('--session_id', default=None)
    args = parser.parse_args()
    log_event(
        args.org_id,
        args.agent_id,
        args.action,
        resource=args.resource,
        outcome=args.outcome,
        details={
            'source': 'loom_runtime_execute',
            'input_hash': args.input_hash,
            'estimated_cost_usd': args.estimated_cost_usd,
            'effective_source': args.effective_source,
            'effective_stage': args.effective_stage,
            'reference_stage': args.reference_stage,
            'runtime_outcome': args.runtime_outcome,
            'worker_status': args.worker_status,
            'worker_kind': args.worker_kind,
            'parity_status': args.parity_status,
            'experimental': True,
        },
        policy_ref='experimental_runtime_rehearsal',
        session_id=args.session_id,
    )

if __name__ == '__main__':
    main()
"#,
        )
        .expect("write audit");
        init_workspace(
            &root,
            "embedded",
            Some(kernel_root.to_string_lossy().as_ref()),
            "org_demo",
        )
        .expect("init workspace");
        let envelope = sample_envelope();
        enqueue_action(&root, &kernel_root, &envelope).expect("enqueue");

        let summary = run_supervisor(&root, Some(kernel_root.to_string_lossy().as_ref()), 1)
            .expect("supervisor");
        assert_eq!(summary.processed, 1);
        assert_eq!(summary.allowed, 1);
        assert_eq!(
            summary.audit_log_path,
            kernel_root
                .join("kernel")
                .join("runtime_audit")
                .join("loom_runtime_events.jsonl")
        );
        assert!(summary.audit_log_path.exists());
        assert!(summary.last_execution_path.exists());
        let pending_dir = root.join(".loom/runtime/queue/pending");
        let processed_dir = root.join(".loom/runtime/queue/processed");
        assert_eq!(
            fs::read_dir(&pending_dir).expect("pending dir").count(),
            0
        );
        assert_eq!(
            fs::read_dir(&processed_dir).expect("processed dir").count(),
            1
        );
        let human = render_supervisor_run_human(&summary);
        assert!(human.contains("experimental local queue supervisor"));
    }

    #[test]
    fn supervisor_watch_writes_heartbeat_and_status() {
        let root = temp_path("loom-shadow-watch");
        fs::create_dir_all(&root).expect("root");
        let kernel = temp_path("loom-shadow-watch-kernel");
        let kernel_dir = kernel.join("kernel");
        fs::create_dir_all(kernel_dir.join("adapters")).expect("kernel dirs");
        fs::write(
            kernel_dir.join("runtimes.json"),
            "{\n  \"runtimes\": {\n    \"local_kernel\": {\"id\": \"local_kernel\", \"label\": \"Local Kernel Runtime\"},\n    \"meridian_loom\": {\"status\": \"experimental\", \"notes\": \"watch fixture\", \"contract_compliance\": {\"agent_identity\": null, \"action_envelope\": null, \"cost_attribution\": null, \"approval_hook\": null, \"audit_emission\": null, \"sanction_controls\": null, \"budget_gate\": null}}\n  }\n}\n",
        )
        .expect("write runtimes");
        fs::write(
            kernel_dir.join("agent_registry.py"),
            "import json, sys\nagent_id = sys.argv[sys.argv.index('--agent_id') + 1]\norg_id = sys.argv[sys.argv.index('--org_id') + 1] if '--org_id' in sys.argv else 'org_demo'\nprint(json.dumps({'id': agent_id, 'name': 'Atlas', 'org_id': org_id, 'role': 'analyst', 'economy_key': 'atlas', 'approval_required': False, 'budget': {'max_per_run_usd': 0.5}, 'runtime_binding': {'runtime_id': 'local_kernel', 'runtime_label': 'Local Kernel Runtime', 'bound_org_id': org_id, 'boundary_name': 'workspace', 'identity_model': 'session', 'runtime_registered': True, 'registration_status': 'registered'}}, indent=2))\n",
        )
        .expect("write registry");
        fs::write(
            kernel_dir.join("court.py"),
            "def get_restrictions(agent_id, org_id=None):\n    return []\n",
        )
        .expect("write court");
        fs::write(
            kernel_dir.join("authority.py"),
            "def check_authority(agent_id, action, org_id=None):\n    return True, 'ok'\n",
        )
        .expect("write authority");
        fs::write(
            kernel_dir.join("treasury.py"),
            "def check_budget(agent_id, cost_usd, org_id=None):\n    return True, 'ok'\n",
        )
        .expect("write treasury");
        fs::write(
            kernel_dir.join("audit.py"),
            "def log_event(*args, **kwargs):\n    return 'evt_watch'\n",
        )
        .expect("write audit");
        fs::write(kernel_dir.join("adapters/__init__.py"), "").expect("write adapter init");
        fs::write(
            kernel_dir.join("adapters/openclaw_compatible.py"),
            "def pre_action_check(org_id, envelope):\n    return {'allowed': True, 'stage': 'ok', 'reason': 'ok', 'restrictions': []}\n",
        )
        .expect("write adapter");
        init_workspace(
            &root,
            "embedded",
            Some(kernel.to_string_lossy().as_ref()),
            "org_demo",
        )
        .expect("init workspace");
        let envelope = sample_envelope();
        enqueue_action(&root, &kernel, &envelope).expect("enqueue");

        let summary = watch_supervisor(
            &root,
            Some(kernel.to_string_lossy().as_ref()),
            1,
            2,
            0,
        )
        .expect("watch supervisor");
        assert_eq!(summary.iterations, 2);
        assert_eq!(summary.processed, 1);
        assert!(summary.heartbeat_log_path.exists());
        assert!(summary.status_path.exists());
        let status = fs::read_to_string(&summary.status_path).expect("status");
        assert!(status.contains("\"status\": \"watch_complete\""));
        let heartbeat = fs::read_to_string(&summary.heartbeat_log_path).expect("heartbeat");
        assert!(heartbeat.contains("\"iteration\":1"));
        assert!(heartbeat.contains("\"iteration\":2"));
        let human = render_supervisor_watch_human(&summary);
        assert!(human.contains("experimental local queue supervisor loop"));
    }

    #[test]
    fn supervisor_status_reads_written_artifacts() {
        let root = temp_path("loom-shadow-supervisor-status");
        fs::create_dir_all(&root).expect("root");
        let kernel = temp_path("loom-shadow-supervisor-status-kernel");
        let kernel_dir = kernel.join("kernel");
        fs::create_dir_all(kernel_dir.join("adapters")).expect("kernel dirs");
        fs::write(
            kernel_dir.join("runtimes.json"),
            "{\n  \"runtimes\": {\n    \"local_kernel\": {\"id\": \"local_kernel\", \"label\": \"Local Kernel Runtime\"},\n    \"meridian_loom\": {\"status\": \"experimental\", \"notes\": \"status fixture\", \"contract_compliance\": {\"agent_identity\": null, \"action_envelope\": null, \"cost_attribution\": null, \"approval_hook\": null, \"audit_emission\": null, \"sanction_controls\": null, \"budget_gate\": null}}\n  }\n}\n",
        )
        .expect("write runtimes");
        fs::write(
            kernel_dir.join("agent_registry.py"),
            "import json, sys\nagent_id = sys.argv[sys.argv.index('--agent_id') + 1]\norg_id = sys.argv[sys.argv.index('--org_id') + 1] if '--org_id' in sys.argv else 'org_demo'\nprint(json.dumps({'id': agent_id, 'name': 'Atlas', 'org_id': org_id, 'role': 'analyst', 'economy_key': 'atlas', 'approval_required': False, 'budget': {'max_per_run_usd': 0.5}, 'runtime_binding': {'runtime_id': 'local_kernel', 'runtime_label': 'Local Kernel Runtime', 'bound_org_id': org_id, 'boundary_name': 'workspace', 'identity_model': 'session', 'runtime_registered': True, 'registration_status': 'registered'}}, indent=2))\n",
        )
        .expect("write registry");
        fs::write(
            kernel_dir.join("court.py"),
            "def get_restrictions(agent_id, org_id=None):\n    return []\n",
        )
        .expect("write court");
        fs::write(
            kernel_dir.join("authority.py"),
            "def check_authority(agent_id, action, org_id=None):\n    return True, 'ok'\n",
        )
        .expect("write authority");
        fs::write(
            kernel_dir.join("treasury.py"),
            "def check_budget(agent_id, cost_usd, org_id=None):\n    return True, 'ok'\n",
        )
        .expect("write treasury");
        fs::write(
            kernel_dir.join("audit.py"),
            "def log_event(*args, **kwargs):\n    return 'evt_status'\n",
        )
        .expect("write audit");
        fs::write(kernel_dir.join("adapters/__init__.py"), "").expect("write adapter init");
        fs::write(
            kernel_dir.join("adapters/openclaw_compatible.py"),
            "def pre_action_check(org_id, envelope):\n    return {'allowed': True, 'stage': 'ok', 'reason': 'ok', 'restrictions': []}\n",
        )
        .expect("write adapter");
        init_workspace(
            &root,
            "embedded",
            Some(kernel.to_string_lossy().as_ref()),
            "org_demo",
        )
        .expect("init workspace");
        let envelope = sample_envelope();
        enqueue_action(&root, &kernel, &envelope).expect("enqueue");
        watch_supervisor(&root, Some(kernel.to_string_lossy().as_ref()), 1, 2, 0)
            .expect("watch supervisor");

        let snapshot = supervisor_status(&root).expect("status");
        assert!(snapshot.available);
        assert_eq!(snapshot.processed, 1);
        assert_eq!(snapshot.allowed, 1);
        assert_eq!(snapshot.heartbeat_entries, 2);
        assert_eq!(snapshot.pending_jobs, 0);
        let human = render_supervisor_status_human(&snapshot);
        assert!(human.contains("SUPERVISOR STATUS"));
        assert!(human.contains("bounded local loop state is real"));
    }

    #[test]
    fn daemon_loop_writes_runtime_state_and_status() {
        let root = temp_path("loom-shadow-daemon-loop");
        fs::create_dir_all(&root).expect("root");
        let kernel = temp_path("loom-shadow-daemon-kernel");
        let kernel_dir = kernel.join("kernel");
        fs::create_dir_all(kernel_dir.join("adapters")).expect("kernel dirs");
        fs::write(
            kernel_dir.join("runtimes.json"),
            "{\n  \"runtimes\": {\n    \"local_kernel\": {\"id\": \"local_kernel\", \"label\": \"Local Kernel Runtime\"},\n    \"meridian_loom\": {\"status\": \"experimental\", \"notes\": \"daemon fixture\", \"contract_compliance\": {\"agent_identity\": null, \"action_envelope\": null, \"cost_attribution\": null, \"approval_hook\": null, \"audit_emission\": null, \"sanction_controls\": null, \"budget_gate\": null}}\n  }\n}\n",
        )
        .expect("write runtimes");
        fs::write(
            kernel_dir.join("agent_registry.py"),
            "import json, sys\nagent_id = sys.argv[sys.argv.index('--agent_id') + 1]\norg_id = sys.argv[sys.argv.index('--org_id') + 1] if '--org_id' in sys.argv else 'org_demo'\nprint(json.dumps({'id': agent_id, 'name': 'Atlas', 'org_id': org_id, 'role': 'analyst', 'economy_key': 'atlas', 'approval_required': False, 'budget': {'max_per_run_usd': 0.5}, 'runtime_binding': {'runtime_id': 'local_kernel', 'runtime_label': 'Local Kernel Runtime', 'bound_org_id': org_id, 'boundary_name': 'workspace', 'identity_model': 'session', 'runtime_registered': True, 'registration_status': 'registered'}}, indent=2))\n",
        )
        .expect("write registry");
        fs::write(
            kernel_dir.join("court.py"),
            "def get_restrictions(agent_id, org_id=None):\n    return []\n",
        )
        .expect("write court");
        fs::write(
            kernel_dir.join("authority.py"),
            "def check_authority(agent_id, action, org_id=None):\n    return True, 'ok'\n",
        )
        .expect("write authority");
        fs::write(
            kernel_dir.join("treasury.py"),
            "def check_budget(agent_id, cost_usd, org_id=None):\n    return True, 'ok'\n",
        )
        .expect("write treasury");
        fs::write(
            kernel_dir.join("audit.py"),
            "def log_event(*args, **kwargs):\n    return 'evt_daemon'\n",
        )
        .expect("write audit");
        fs::write(kernel_dir.join("adapters/__init__.py"), "").expect("write adapter init");
        fs::write(
            kernel_dir.join("adapters/openclaw_compatible.py"),
            "def pre_action_check(org_id, envelope):\n    return {'allowed': True, 'stage': 'ok', 'reason': 'ok', 'restrictions': []}\n",
        )
        .expect("write adapter");
        init_workspace(
            &root,
            "embedded",
            Some(kernel.to_string_lossy().as_ref()),
            "org_demo",
        )
        .expect("init workspace");
        let envelope = sample_envelope();
        enqueue_action(&root, &kernel, &envelope).expect("enqueue");

        let snapshot = run_supervisor_daemon_loop(
            &root,
            Some(kernel.to_string_lossy().as_ref()),
            1,
            0,
            2,
            "daemon-test",
        )
        .expect("daemon loop");
        assert!(snapshot.available);
        assert_eq!(snapshot.session_id, "daemon-test");
        assert_eq!(snapshot.processed, 1);
        assert_eq!(snapshot.status, "completed");
        assert!(snapshot.runtime_state_path.exists());
        let human = render_supervisor_daemon_human(&snapshot);
        assert!(human.contains("SUPERVISOR DAEMON"));
    }

    fn temp_path(prefix: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "{}-{}",
            prefix,
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ))
    }

    fn sample_identity() -> AgentIdentityResolution {
        AgentIdentityResolution {
            agent_id: "agent_atlas".to_string(),
            agent_name: "Atlas".to_string(),
            org_id: "org_demo".to_string(),
            role: "analyst".to_string(),
            economy_key: "atlas".to_string(),
            approval_required: false,
            max_per_run_usd: Some(0.5),
            restrictions: vec![],
            sanction_decision: "clear".to_string(),
            runtime_id: "local_kernel".to_string(),
            runtime_label: "Local Kernel Runtime".to_string(),
            bound_org_id: "org_demo".to_string(),
            boundary_name: "workspace".to_string(),
            identity_model: "session".to_string(),
            runtime_registered: true,
            registration_status: "registered".to_string(),
            source: "kernel_agent_registry".to_string(),
        }
    }

    fn sample_envelope() -> ActionEnvelope {
        ActionEnvelope {
            agent_id: "agent_atlas".to_string(),
            agent_name: "Atlas".to_string(),
            org_id: "org_demo".to_string(),
            runtime_id: "local_kernel".to_string(),
            runtime_label: "Local Kernel Runtime".to_string(),
            action_type: "research".to_string(),
            resource: "web_search".to_string(),
            estimated_cost_usd: 0.25,
            run_id: "run_1".to_string(),
            session_id: "session_1".to_string(),
            source: "loom_experimental_preflight".to_string(),
        }
    }
}
