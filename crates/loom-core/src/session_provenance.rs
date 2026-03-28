use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};

pub type LoomResult<T> = Result<T, String>;

pub const DEFAULT_SESSION_PROVENANCE_REGISTRY_PATH: &str =
    "state/session-provenance/registry.json";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionProvenanceRecord {
    pub session_key: String,
    pub channel_id: String,
    pub peer_id: String,
    pub agent_id: String,
    pub binding_id: String,
    pub provider_profile: String,
    pub model: String,
    pub ingress_request_id: Option<String>,
    pub job_id: Option<String>,
    pub delivery_id: Option<String>,
    pub override_source: String,
    pub send_policy: String,
    pub opened_at: String,
    pub last_active_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionProvenanceOverview {
    pub registry_path: PathBuf,
    pub total_count: usize,
    pub session_keys: Vec<String>,
}

pub fn session_provenance_registry_path(root: &Path) -> PathBuf {
    root.join(DEFAULT_SESSION_PROVENANCE_REGISTRY_PATH)
}

pub fn ensure_session_provenance_scaffold(root: &Path) -> LoomResult<PathBuf> {
    let registry_path = session_provenance_registry_path(root);
    if let Some(parent) = registry_path.parent() {
        fs::create_dir_all(parent).map_err(io_err)?;
    }
    if !registry_path.exists() {
        fs::write(&registry_path, "{\n  \"sessions\": []\n}\n").map_err(io_err)?;
    }
    Ok(registry_path)
}

/// Open or refresh a provenance record for a session.
/// If the session_key already exists, updates last_active_at.
/// If it does not exist, creates a new record.
pub fn open_session_provenance(
    root: &Path,
    session_key: &str,
    channel_id: &str,
    peer_id: &str,
    agent_id: &str,
    binding_id: &str,
) -> LoomResult<SessionProvenanceRecord> {
    let ts = timestamp_now();
    let mut records = load_session_provenance_records(root)?;
    if let Some(existing) = records.iter_mut().find(|r| r.session_key == session_key) {
        existing.last_active_at = ts.clone();
        existing.agent_id = agent_id.to_string();
        let updated = existing.clone();
        persist_session_provenance_registry(root, &records)?;
        return Ok(updated);
    }
    let record = SessionProvenanceRecord {
        session_key: session_key.to_string(),
        channel_id: channel_id.to_string(),
        peer_id: peer_id.to_string(),
        agent_id: agent_id.to_string(),
        binding_id: binding_id.to_string(),
        provider_profile: String::new(),
        model: String::new(),
        ingress_request_id: None,
        job_id: None,
        delivery_id: None,
        override_source: "default".to_string(),
        send_policy: "deliver".to_string(),
        opened_at: ts.clone(),
        last_active_at: ts,
    };
    records.push(record.clone());
    persist_session_provenance_registry(root, &records)?;
    Ok(record)
}

/// Update the resolved provider route info on a provenance record.
pub fn update_session_provenance_route(
    root: &Path,
    session_key: &str,
    provider_profile: &str,
    model: &str,
    override_source: &str,
) -> LoomResult<()> {
    let ts = timestamp_now();
    let mut records = load_session_provenance_records(root)?;
    if let Some(record) = records.iter_mut().find(|r| r.session_key == session_key) {
        if !provider_profile.is_empty() {
            record.provider_profile = provider_profile.to_string();
        }
        if !model.is_empty() {
            record.model = model.to_string();
        }
        if !override_source.is_empty() {
            record.override_source = override_source.to_string();
        }
        record.last_active_at = ts;
        persist_session_provenance_registry(root, &records)?;
    }
    Ok(())
}

/// Link a job_id and/or delivery_id and/or ingress_request_id to a provenance record.
pub fn update_session_provenance_job(
    root: &Path,
    session_key: &str,
    job_id: Option<&str>,
    delivery_id: Option<&str>,
    ingress_request_id: Option<&str>,
) -> LoomResult<()> {
    let ts = timestamp_now();
    let mut records = load_session_provenance_records(root)?;
    if let Some(record) = records.iter_mut().find(|r| r.session_key == session_key) {
        if let Some(j) = job_id {
            if !j.is_empty() {
                record.job_id = Some(j.to_string());
            }
        }
        if let Some(d) = delivery_id {
            if !d.is_empty() {
                record.delivery_id = Some(d.to_string());
            }
        }
        if let Some(i) = ingress_request_id {
            if !i.is_empty() {
                record.ingress_request_id = Some(i.to_string());
            }
        }
        record.last_active_at = ts;
        persist_session_provenance_registry(root, &records)?;
    }
    Ok(())
}

pub fn find_session_provenance(
    root: &Path,
    session_key: &str,
) -> LoomResult<Option<SessionProvenanceRecord>> {
    let session_key = session_key.trim();
    if session_key.is_empty() {
        return Err("session_key is required".to_string());
    }
    let records = load_session_provenance_records(root)?;
    Ok(records.into_iter().find(|r| r.session_key == session_key))
}

pub fn list_session_provenance(
    root: &Path,
    limit: usize,
) -> LoomResult<Vec<SessionProvenanceRecord>> {
    let mut records = load_session_provenance_records(root)?;
    records.sort_by(|a, b| b.last_active_at.cmp(&a.last_active_at));
    if limit > 0 && records.len() > limit {
        records.truncate(limit);
    }
    Ok(records)
}

pub fn session_provenance_overview(root: &Path) -> LoomResult<SessionProvenanceOverview> {
    let records = load_session_provenance_records(root)?;
    Ok(SessionProvenanceOverview {
        registry_path: session_provenance_registry_path(root),
        total_count: records.len(),
        session_keys: records.iter().map(|r| r.session_key.clone()).collect(),
    })
}

pub fn sync_session_provenance_registry(root: &Path) -> LoomResult<()> {
    ensure_session_provenance_scaffold(root)?;
    Ok(())
}

// --- render ---

pub fn render_session_provenance_overview_human(overview: &SessionProvenanceOverview) -> String {
    format!(
        "registry_path:   {}\ntotal_count:     {}\nsessions:        {}\n",
        overview.registry_path.display(),
        overview.total_count,
        if overview.session_keys.is_empty() {
            "(none)".to_string()
        } else {
            overview.session_keys.join(",")
        }
    )
}

pub fn render_session_provenance_overview_json(overview: &SessionProvenanceOverview) -> String {
    serde_json::to_string_pretty(&json!({
        "registry_path": overview.registry_path.display().to_string(),
        "total_count": overview.total_count,
        "session_keys": overview.session_keys,
    }))
    .unwrap_or_else(|_| "{}".to_string())
        + "\n"
}

pub fn render_session_provenance_human(record: &SessionProvenanceRecord) -> String {
    format!(
        "session_key:          {}\nchannel_id:           {}\npeer_id:              {}\nagent_id:             {}\nbinding_id:           {}\nprovider_profile:     {}\nmodel:                {}\ningress_request_id:   {}\njob_id:               {}\ndelivery_id:          {}\noverride_source:      {}\nsend_policy:          {}\nopened_at:            {}\nlast_active_at:       {}\n",
        record.session_key,
        record.channel_id,
        record.peer_id,
        record.agent_id,
        record.binding_id,
        if record.provider_profile.is_empty() { "(none)" } else { &record.provider_profile },
        if record.model.is_empty() { "(none)" } else { &record.model },
        record.ingress_request_id.as_deref().unwrap_or("(none)"),
        record.job_id.as_deref().unwrap_or("(none)"),
        record.delivery_id.as_deref().unwrap_or("(none)"),
        record.override_source,
        record.send_policy,
        record.opened_at,
        record.last_active_at,
    )
}

pub fn render_session_provenance_json(record: &SessionProvenanceRecord) -> String {
    serde_json::to_string_pretty(&session_provenance_record_json(record))
        .unwrap_or_else(|_| "{}".to_string())
        + "\n"
}

pub fn render_session_provenance_list_human(records: &[SessionProvenanceRecord]) -> String {
    if records.is_empty() {
        return "session_count:     0\n".to_string();
    }
    let mut out = format!("session_count:     {}\n", records.len());
    for r in records {
        out.push_str(&format!(
            "\n- {} channel={} peer={} agent={} policy={}\n",
            r.session_key, r.channel_id, r.peer_id, r.agent_id, r.send_policy
        ));
    }
    out
}

pub fn render_session_provenance_list_json(records: &[SessionProvenanceRecord]) -> String {
    serde_json::to_string_pretty(
        &records
            .iter()
            .map(session_provenance_record_json)
            .collect::<Vec<_>>(),
    )
    .unwrap_or_else(|_| "[]".to_string())
        + "\n"
}

// --- internal ---

fn load_session_provenance_records(root: &Path) -> LoomResult<Vec<SessionProvenanceRecord>> {
    ensure_session_provenance_scaffold(root)?;
    let raw =
        fs::read_to_string(session_provenance_registry_path(root)).map_err(io_err)?;
    parse_session_provenance_registry(&raw)
}

fn persist_session_provenance_registry(
    root: &Path,
    records: &[SessionProvenanceRecord],
) -> LoomResult<()> {
    let registry_path = session_provenance_registry_path(root);
    if let Some(parent) = registry_path.parent() {
        fs::create_dir_all(parent).map_err(io_err)?;
    }
    let value = json!({
        "sessions": records.iter().map(session_provenance_record_json).collect::<Vec<_>>()
    });
    let mut rendered =
        serde_json::to_string_pretty(&value).map_err(|e| e.to_string())?;
    rendered.push('\n');
    fs::write(registry_path, rendered).map_err(io_err)
}

fn parse_session_provenance_registry(raw: &str) -> LoomResult<Vec<SessionProvenanceRecord>> {
    let value: Value = serde_json::from_str(raw)
        .map_err(|e| format!("invalid session provenance registry json: {e}"))?;
    let sessions = value
        .get("sessions")
        .and_then(Value::as_array)
        .ok_or_else(|| "session provenance registry must define a sessions array".to_string())?;
    let mut records = Vec::with_capacity(sessions.len());
    for s in sessions {
        records.push(parse_session_provenance_record(s)?);
    }
    Ok(records)
}

fn parse_session_provenance_record(value: &Value) -> LoomResult<SessionProvenanceRecord> {
    Ok(SessionProvenanceRecord {
        session_key: value_string(value.get("session_key"), "session_key")?,
        channel_id: value_string_or(value.get("channel_id"), ""),
        peer_id: value_string_or(value.get("peer_id"), ""),
        agent_id: value_string_or(value.get("agent_id"), ""),
        binding_id: value_string_or(value.get("binding_id"), ""),
        provider_profile: value_string_or(value.get("provider_profile"), ""),
        model: value_string_or(value.get("model"), ""),
        ingress_request_id: value_opt_string(value.get("ingress_request_id")),
        job_id: value_opt_string(value.get("job_id")),
        delivery_id: value_opt_string(value.get("delivery_id")),
        override_source: value_string_or(value.get("override_source"), "default"),
        send_policy: value_string_or(value.get("send_policy"), "deliver"),
        opened_at: value_string_or(value.get("opened_at"), ""),
        last_active_at: value_string_or(value.get("last_active_at"), ""),
    })
}

fn session_provenance_record_json(record: &SessionProvenanceRecord) -> Value {
    json!({
        "session_key": record.session_key,
        "channel_id": record.channel_id,
        "peer_id": record.peer_id,
        "agent_id": record.agent_id,
        "binding_id": record.binding_id,
        "provider_profile": record.provider_profile,
        "model": record.model,
        "ingress_request_id": record.ingress_request_id,
        "job_id": record.job_id,
        "delivery_id": record.delivery_id,
        "override_source": record.override_source,
        "send_policy": record.send_policy,
        "opened_at": record.opened_at,
        "last_active_at": record.last_active_at,
    })
}

fn timestamp_now() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("{}", secs)
}

fn io_err(error: std::io::Error) -> String {
    error.to_string()
}

fn value_string(value: Option<&Value>, label: &str) -> LoomResult<String> {
    value
        .and_then(Value::as_str)
        .map(|raw| raw.trim().to_string())
        .filter(|raw| !raw.is_empty())
        .ok_or_else(|| format!("{label} must not be empty"))
}

fn value_string_or(value: Option<&Value>, fallback: &str) -> String {
    value
        .and_then(Value::as_str)
        .map(|raw| raw.trim().to_string())
        .unwrap_or_else(|| fallback.to_string())
}

fn value_opt_string(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .map(|raw| raw.trim().to_string())
        .filter(|raw| !raw.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::init_workspace;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path(label: &str) -> std::path::PathBuf {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!("{}-{}", label, ts))
    }

    #[test]
    fn open_session_provenance_creates_record() {
        let root = temp_path("loom-session-prov-create");
        init_workspace(&root, "embedded", None, "org_demo").expect("init");
        let record = open_session_provenance(
            &root,
            "telegram:founder",
            "telegram",
            "founder",
            "leviathann",
            "binding-telegram",
        )
        .expect("open provenance");
        assert_eq!(record.session_key, "telegram:founder");
        assert_eq!(record.channel_id, "telegram");
        assert_eq!(record.agent_id, "leviathann");
        assert_eq!(record.send_policy, "deliver");

        let records = list_session_provenance(&root, 10).expect("list");
        assert_eq!(records.len(), 1);
    }

    #[test]
    fn open_session_provenance_updates_existing() {
        let root = temp_path("loom-session-prov-update");
        init_workspace(&root, "embedded", None, "org_demo").expect("init");
        open_session_provenance(
            &root,
            "telegram:founder",
            "telegram",
            "founder",
            "leviathann",
            "binding-telegram",
        )
        .expect("open first");
        open_session_provenance(
            &root,
            "telegram:founder",
            "telegram",
            "founder",
            "atlas",
            "binding-telegram",
        )
        .expect("open second");

        let records = list_session_provenance(&root, 10).expect("list");
        assert_eq!(records.len(), 1, "must not duplicate");
        assert_eq!(records[0].agent_id, "atlas");
    }

    #[test]
    fn find_session_provenance_returns_none_for_unknown() {
        let root = temp_path("loom-session-prov-find");
        init_workspace(&root, "embedded", None, "org_demo").expect("init");
        let result = find_session_provenance(&root, "nope:nope").expect("find");
        assert!(result.is_none());
    }

    #[test]
    fn update_session_provenance_job_links_ids() {
        let root = temp_path("loom-session-prov-job");
        init_workspace(&root, "embedded", None, "org_demo").expect("init");
        open_session_provenance(
            &root,
            "web_api:user42",
            "web_api",
            "user42",
            "leviathann",
            "binding-web_api",
        )
        .expect("open");
        update_session_provenance_job(
            &root,
            "web_api:user42",
            Some("job-abc"),
            Some("delivery-xyz"),
            Some("req-123"),
        )
        .expect("update job");
        let record = find_session_provenance(&root, "web_api:user42")
            .expect("find")
            .expect("record must exist");
        assert_eq!(record.job_id.as_deref(), Some("job-abc"));
        assert_eq!(record.delivery_id.as_deref(), Some("delivery-xyz"));
        assert_eq!(record.ingress_request_id.as_deref(), Some("req-123"));
    }
}
