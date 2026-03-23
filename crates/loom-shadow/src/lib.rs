use loom_core::{
    envelope_input_hash, preview_local_sanction_controls, ActionEnvelope, AgentIdentityResolution,
    ReferenceGateCheck,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

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
    pub audit_log_path: PathBuf,
    pub parity_stream_path: PathBuf,
    pub parity_report_path: PathBuf,
    pub openclaw_live_probe_path: Option<PathBuf>,
    pub decision_path: PathBuf,
    pub input_hash: String,
    pub agent_id: String,
    pub org_id: String,
    pub action_type: String,
    pub resource: String,
    pub estimated_cost_usd: f64,
    pub runtime_outcome: String,
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
    let audit_log_path = ensure_audit_dir(root)?.join("runtime_events.jsonl");
    let execution_path = runtime_dir.join("last_execution.json");
    let parity_stream_path = parity_dir.join("stream.jsonl");
    let parity_report_path = parity_dir.join("latest.json");
    let (openclaw_live_probe_path, openclaw_live_probe_status, openclaw_live_probe_note) =
        capture_openclaw_live_probe(root)?;
    let runtime_outcome = if decision.overall_decision == "allow" {
        "simulated_success".to_string()
    } else {
        "denied".to_string()
    };
    let audit_emission_status = emit_runtime_audit(
        kernel_path,
        &audit_log_path,
        envelope,
        decision,
        &runtime_outcome,
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
            "{{\"timestamp\":{},\"source\":\"loom_runtime_stream\",\"phase\":\"native_enforcement\",\"hook_name\":{},\"decision\":{},\"stage\":{},\"agent_id\":{},\"org_id\":{},\"input_hash\":{},\"reason\":{}}}\n",
            json_string(&stream_timestamp),
            json_string(&decision.effective_stage),
            json_string(&decision.overall_decision),
            json_string(&decision.effective_stage),
            json_string(&decision.agent_id),
            json_string(&decision.org_id),
            json_string(&decision.input_hash),
            json_string(&decision.effective_reason),
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
            "{{\"timestamp\":{},\"source\":\"openclaw_live_probe\",\"phase\":\"live_runtime_probe\",\"hook_name\":\"runtime_health\",\"decision\":{},\"stage\":\"live_single_host_openclaw\",\"agent_id\":{},\"org_id\":{},\"input_hash\":{},\"reason\":{},\"probe_path\":{}}}\n",
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
        ),
    )?;

    let capture = RuntimeExecutionCapture {
        execution_path: execution_path.clone(),
        audit_log_path: audit_log_path.clone(),
        parity_stream_path: parity_stream_path.clone(),
        parity_report_path: parity_report_path.clone(),
        openclaw_live_probe_path,
        decision_path: decision.decision_path.clone(),
        input_hash: decision.input_hash.clone(),
        agent_id: decision.agent_id.clone(),
        org_id: decision.org_id.clone(),
        action_type: decision.action_type.clone(),
        resource: decision.resource.clone(),
        estimated_cost_usd: decision.estimated_cost_usd,
        runtime_outcome,
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
    Ok(capture)
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
    format!(
        "Meridian Loom // RUNTIME EXECUTE\n=================================\nexecution_path:         {}\ndecision_path:          {}\naudit_log:              {}\nparity_stream:          {}\nparity_report:          {}\nopenclaw_live_probe:    {}\nagent_id:               {}\norg_id:                 {}\naction_type:            {}\nresource:               {}\ninput_hash:             {}\nestimated_cost_usd:{:>12.4}\nruntime_outcome:        {}\noverall_decision:       {}\neffective_source:       {}\neffective_stage:        {}\nreference_decision:     {}\nreference_stage:        {}\naudit_emission:         {}\nopenclaw_probe:         {} ({})\nparity_status:          {}\nparity_reason:          {}\n",
        capture.execution_path.display(),
        capture.decision_path.display(),
        capture.audit_log_path.display(),
        capture.parity_stream_path.display(),
        capture.parity_report_path.display(),
        capture
            .openclaw_live_probe_path
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "(not captured)".to_string()),
        capture.agent_id,
        capture.org_id,
        capture.action_type,
        capture.resource,
        capture.input_hash,
        capture.estimated_cost_usd,
        capture.runtime_outcome,
        capture.overall_decision,
        capture.effective_source,
        capture.effective_stage,
        capture.reference_decision,
        capture.reference_stage,
        capture.audit_emission_status,
        capture.openclaw_live_probe_status,
        capture.openclaw_live_probe_note,
        capture.parity_status,
        capture.parity_reason,
    )
}

pub fn render_runtime_execution_json(capture: &RuntimeExecutionCapture) -> String {
    format!(
        "{{\n  \"status\": \"runtime_execution_captured\",\n  \"execution_path\": {},\n  \"decision_path\": {},\n  \"audit_log_path\": {},\n  \"parity_stream_path\": {},\n  \"parity_report_path\": {},\n  \"openclaw_live_probe_path\": {},\n  \"agent_id\": {},\n  \"org_id\": {},\n  \"action_type\": {},\n  \"resource\": {},\n  \"input_hash\": {},\n  \"estimated_cost_usd\": {:.6},\n  \"runtime_outcome\": {},\n  \"overall_decision\": {},\n  \"effective_source\": {},\n  \"effective_stage\": {},\n  \"reference_decision\": {},\n  \"reference_stage\": {},\n  \"audit_emission_status\": {},\n  \"openclaw_live_probe_status\": {},\n  \"openclaw_live_probe_note\": {},\n  \"parity_status\": {},\n  \"parity_reason\": {},\n  \"note\": \"experimental runtime rehearsal only; no governed worker supervisor yet\"\n}}\n",
        json_string(&capture.execution_path.display().to_string()),
        json_string(&capture.decision_path.display().to_string()),
        json_string(&capture.audit_log_path.display().to_string()),
        json_string(&capture.parity_stream_path.display().to_string()),
        json_string(&capture.parity_report_path.display().to_string()),
        capture
            .openclaw_live_probe_path
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
    let mut out = String::from("Meridian Loom // SHADOW REPORT\n==============================\n");
    let stale_latest = contents
        .as_ref()
        .map(|value| value.contains("\"status\": \"not_started\""))
        .unwrap_or(false);

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
    let contents = fs::read_to_string(&report_path)
        .map_err(|_| format!("could not read parity report at {}", report_path.display()))?;
    let stream_path = root.join(".loom/parity/stream.jsonl");
    let stream = fs::read_to_string(&stream_path)
        .map_err(|_| format!("could not read parity stream at {}", stream_path.display()))?;
    let openclaw_live_path = root.join(".loom/parity/openclaw_live.json");
    let openclaw_live = fs::read_to_string(&openclaw_live_path).ok();
    Ok(format!(
        "Meridian Loom // PARITY REPORT\n===============================\nsource: {}\n\n{}\nParity stream\n-------------\nsource: {}\n\n{}\n{}\n",
        report_path.display(),
        contents,
        stream_path.display(),
        stream,
        openclaw_live
            .map(|contents| format!(
                "OpenClaw live probe\n-------------------\nsource: {}\n\n{}\n",
                openclaw_live_path.display(),
                contents
            ))
            .unwrap_or_else(|| "OpenClaw live probe\n-------------------\nsource: (not captured)\n\n".to_string()),
    ))
}

fn render_parity_report_json(capture: &RuntimeExecutionCapture) -> String {
    format!(
        "{{\n  \"status\": \"parity_report_captured\",\n  \"execution_path\": {},\n  \"decision_path\": {},\n  \"audit_log_path\": {},\n  \"parity_stream_path\": {},\n  \"openclaw_live_probe_path\": {},\n  \"reference_decision\": {},\n  \"reference_stage\": {},\n  \"overall_decision\": {},\n  \"effective_stage\": {},\n  \"openclaw_live_probe_status\": {},\n  \"openclaw_live_probe_note\": {},\n  \"parity_status\": {},\n  \"parity_reason\": {},\n  \"note\": \"runtime-side parity stream now captures Loom execution plus an OpenClaw live proof snapshot when available; per-action live parity remains future work\"\n}}\n",
        json_string(&capture.execution_path.display().to_string()),
        json_string(&capture.decision_path.display().to_string()),
        json_string(&capture.audit_log_path.display().to_string()),
        json_string(&capture.parity_stream_path.display().to_string()),
        capture
            .openclaw_live_probe_path
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

fn ensure_audit_dir(root: &Path) -> ShadowResult<PathBuf> {
    let audit_dir = root.join(".loom/audit");
    fs::create_dir_all(&audit_dir).map_err(io_err)?;
    Ok(audit_dir)
}

fn ensure_parity_dir(root: &Path) -> ShadowResult<PathBuf> {
    let parity_dir = root.join(".loom/parity");
    fs::create_dir_all(&parity_dir).map_err(io_err)?;
    Ok(parity_dir)
}

fn capture_openclaw_live_probe(root: &Path) -> ShadowResult<(Option<PathBuf>, String, String)> {
    let parity_dir = ensure_parity_dir(root)?;
    let probe_path = parity_dir.join("openclaw_live.json");
    let proof_script = std::env::var("MERIDIAN_OPENCLAW_PROOF_SCRIPT")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from("/root/.openclaw/workspace/company/meridian_platform/openclaw_runtime_proof.py")
        });
    if !proof_script.exists() {
        return Ok((
            None,
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
        return Ok((
            None,
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
    Ok((
        Some(probe_path),
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
outcome = sys.argv[9]
effective_source = sys.argv[10]
effective_stage = sys.argv[11]
reference_stage = sys.argv[12]
sys.path.insert(0, kernel_dir)
import audit
event_id = audit.log_event(
    org_id,
    agent_id,
    action,
    resource=resource,
    outcome=outcome,
    actor_type='agent',
    details={
        'source': 'loom_runtime_execute',
        'input_hash': input_hash,
        'estimated_cost_usd': estimated_cost,
        'effective_source': effective_source,
        'effective_stage': effective_stage,
        'reference_stage': reference_stage,
        'experimental': True,
    },
    policy_ref='experimental_runtime_rehearsal',
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
            .arg(&decision.input_hash)
            .arg(format!("{:.6}", envelope.estimated_cost_usd))
            .arg(&envelope.session_id)
            .arg(outcome)
            .arg(&decision.effective_source)
            .arg(&decision.effective_stage)
            .arg(&decision.reference_stage)
            .env("MERIDIAN_AUDIT_FILE", audit_log_path)
            .output()
            .map_err(io_err)?;
        if output.status.success() {
            return Ok("runtime_event_written".to_string());
        }
    }

    append_line(
        audit_log_path,
        &format!(
            "{{\"id\":{},\"timestamp\":{},\"org_id\":{},\"agent_id\":{},\"actor_type\":\"agent\",\"action\":{},\"resource\":{},\"outcome\":{},\"details\":{{\"source\":\"loom_runtime_execute\",\"input_hash\":{},\"estimated_cost_usd\":{:.6},\"effective_source\":{},\"effective_stage\":{},\"reference_stage\":{},\"experimental\":true}},\"policy_ref\":\"experimental_runtime_rehearsal\"}}\n",
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
    use loom_core::{ActionEnvelope, AgentIdentityResolution};

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
        assert!(capture.audit_log_path.exists());
        assert!(capture.parity_stream_path.exists());
        assert!(capture.parity_report_path.exists());
        assert_eq!(capture.runtime_outcome, "simulated_success");
        assert_eq!(capture.reference_decision, "allow");
        assert_eq!(capture.parity_status, "match");

        let human = render_runtime_execution_human(&capture);
        assert!(human.contains("Meridian Loom // RUNTIME EXECUTE"));
        assert!(human.contains("openclaw_probe:"));
        let json = render_runtime_execution_json(&capture);
        assert!(json.contains("\"status\": \"runtime_execution_captured\""));
        assert!(json.contains("\"openclaw_live_probe_status\":"));
        let parity = render_parity_report(&root).expect("parity report");
        assert!(parity.contains("Meridian Loom // PARITY REPORT"));
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
