use loom_core::{envelope_input_hash, ActionEnvelope, AgentIdentityResolution};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub type ShadowResult<T> = Result<T, String>;

#[derive(Clone, Debug, PartialEq)]
pub struct PreflightCapture {
    pub event_log: PathBuf,
    pub latest_report: PathBuf,
    pub input_hash: String,
    pub hooks: Vec<String>,
    pub estimated_cost_usd: f64,
    pub budget_limit_usd: Option<f64>,
    pub budget_gate_decision: String,
    pub approval_decision: String,
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
    pub note: String,
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
    identity: &AgentIdentityResolution,
    envelope: &ActionEnvelope,
) -> ShadowResult<PreflightCapture> {
    let shadow_dir = ensure_shadow_dir(root)?;
    let event_log = shadow_dir.join("events.jsonl");
    let latest_report = shadow_dir.join("latest.json");
    let input_hash = envelope_input_hash(envelope);
    let budget_limit_usd = identity.max_per_run_usd;
    let budget_gate_decision = match budget_limit_usd {
        Some(limit) if envelope.estimated_cost_usd <= limit => "allow",
        Some(_) => "deny",
        None => "unknown",
    };
    let approval_decision = if identity.approval_required {
        "requires_approval"
    } else {
        "not_required"
    };

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
            json_string(approval_decision),
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
            json_string(budget_gate_decision),
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

    fs::write(
        &latest_report,
        format!(
            "{{\n  \"status\": \"preflight_captured\",\n  \"events_compared\": 0,\n  \"divergences\": 0,\n  \"captured_hooks\": [\"agent_identity\", \"action_envelope\", \"cost_attribution\", \"approval_hook\", \"budget_gate\"],\n  \"input_hash\": {},\n  \"estimated_cost_usd\": {:.6},\n  \"budget_limit_usd\": {},\n  \"budget_gate_decision\": {},\n  \"approval_decision\": {},\n  \"event_log\": {},\n  \"note\": \"experimental preflight captured; no primary comparison run yet\"\n}}\n",
            json_string(&input_hash),
            envelope.estimated_cost_usd,
            budget_limit_usd
                .map(|value| format!("{:.6}", value))
                .unwrap_or_else(|| "null".to_string()),
            json_string(budget_gate_decision),
            json_string(approval_decision),
            json_string(&event_log.display().to_string()),
        ),
    )
    .map_err(io_err)?;

    Ok(PreflightCapture {
        event_log,
        latest_report,
        input_hash,
        hooks: vec![
            "agent_identity".to_string(),
            "action_envelope".to_string(),
            "cost_attribution".to_string(),
            "approval_hook".to_string(),
            "budget_gate".to_string(),
        ],
        estimated_cost_usd: envelope.estimated_cost_usd,
        budget_limit_usd,
        budget_gate_decision: budget_gate_decision.to_string(),
        approval_decision: approval_decision.to_string(),
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

    for idx in 0..pairs_compared {
        let left = &primary_events[idx];
        let right = &shadow_events[idx];
        if left.hook_name == right.hook_name
            && left.input_hash == right.input_hash
            && left.decision == right.decision
            && left.agent_id == right.agent_id
            && left.org_id == right.org_id
        {
            matches += 1;
        } else {
            divergences += 1;
        }
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
        note: "file-level comparison only; not proof of runtime parity".to_string(),
    };

    if let Some(root) = root {
        let latest_report = ensure_shadow_dir(root)?.join("latest.json");
        fs::write(&latest_report, render_compare_json(&summary)).map_err(io_err)?;
    }

    Ok(summary)
}

pub fn render_preflight_human(capture: &PreflightCapture) -> String {
    format!(
        "Shadow preflight capture\n========================\nevent_log:           {}\nlatest_report:       {}\ninput_hash:          {}\nestimated_cost_usd:  {:.4}\nbudget_limit_usd:    {}\nbudget_gate:         {}\napproval_hook:       {}\ncaptured_hooks:      {}\n",
        capture.event_log.display(),
        capture.latest_report.display(),
        capture.input_hash,
        capture.estimated_cost_usd,
        capture
            .budget_limit_usd
            .map(|value| format!("{:.4}", value))
            .unwrap_or_else(|| "(unknown)".to_string()),
        capture.budget_gate_decision,
        capture.approval_decision,
        capture.hooks.join(", ")
    )
}

pub fn render_preflight_json(capture: &PreflightCapture) -> String {
    format!(
        "{{\n  \"event_log\": {},\n  \"latest_report\": {},\n  \"input_hash\": {},\n  \"estimated_cost_usd\": {:.6},\n  \"budget_limit_usd\": {},\n  \"budget_gate_decision\": {},\n  \"approval_decision\": {},\n  \"captured_hooks\": [\"agent_identity\", \"action_envelope\", \"cost_attribution\", \"approval_hook\", \"budget_gate\"]\n}}\n",
        json_string(&capture.event_log.display().to_string()),
        json_string(&capture.latest_report.display().to_string()),
        json_string(&capture.input_hash),
        capture.estimated_cost_usd,
        capture
            .budget_limit_usd
            .map(|value| format!("{:.6}", value))
            .unwrap_or_else(|| "null".to_string()),
        json_string(&capture.budget_gate_decision),
        json_string(&capture.approval_decision),
    )
}

pub fn render_compare_human(summary: &ComparisonSummary) -> String {
    format!(
        "Shadow comparison\n=================\nprimary_log:     {}\nshadow_log:      {}\nprimary_events:  {}\nshadow_events:   {}\npairs_compared:  {}\nmatches:         {}\ndivergences:     {}\ndivergence_rate: {:.4}\nnote:            {}\n",
        summary.primary_log.display(),
        summary.shadow_log.display(),
        summary.primary_events,
        summary.shadow_events,
        summary.pairs_compared,
        summary.matches,
        summary.divergences,
        summary.divergence_rate,
        summary.note,
    )
}

pub fn render_compare_json(summary: &ComparisonSummary) -> String {
    format!(
        "{{\n  \"status\": \"comparison_complete\",\n  \"primary_log\": {},\n  \"shadow_log\": {},\n  \"primary_events\": {},\n  \"shadow_events\": {},\n  \"pairs_compared\": {},\n  \"matches\": {},\n  \"divergences\": {},\n  \"divergence_rate\": {:.6},\n  \"note\": {}\n}}\n",
        json_string(&summary.primary_log.display().to_string()),
        json_string(&summary.shadow_log.display().to_string()),
        summary.primary_events,
        summary.shadow_events,
        summary.pairs_compared,
        summary.matches,
        summary.divergences,
        summary.divergence_rate,
        json_string(&summary.note),
    )
}

pub fn render_shadow_report(root: &Path) -> ShadowResult<String> {
    let report_path = root.join(".loom/shadow/latest.json");
    let contents = fs::read_to_string(&report_path)
        .map_err(|error| format!("could not read {}: {}", report_path.display(), error))?;
    Ok(format!(
        "Shadow report\n=============\nsource: {}\n\n{}\n",
        report_path.display(),
        contents
    ))
}

fn ensure_shadow_dir(root: &Path) -> ShadowResult<PathBuf> {
    let shadow_dir = root.join(".loom/shadow");
    fs::create_dir_all(&shadow_dir).map_err(io_err)?;
    Ok(shadow_dir)
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

fn json_string(input: &str) -> String {
    format!("{:?}", input)
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

        let capture = capture_preflight(&root, &identity, &envelope).expect("capture");
        assert!(capture.event_log.exists());
        assert_eq!(capture.budget_gate_decision, "allow");
        assert_eq!(capture.approval_decision, "not_required");
        let report = render_shadow_report(&root).expect("report");
        assert!(report.contains("preflight_captured"));
        assert!(report.contains("budget_gate"));
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
        let report = render_shadow_report(&root).expect("render report");
        assert!(report.contains("comparison_complete"));
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
