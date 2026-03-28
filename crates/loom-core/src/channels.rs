use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};

use crate::onboarding::{bind_host_for, load_onboard_manifest, OnboardManifest};
use crate::output_guard::{guard_user_visible_output, OutputGuardPolicy};

pub type LoomResult<T> = Result<T, String>;

pub const DEFAULT_CHANNEL_REGISTRY_PATH: &str = "state/channels/registry.json";
pub const DEFAULT_CHANNEL_DELIVERY_DIR: &str = "state/channels/delivery";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChannelRecord {
    pub channel_id: String,
    pub kind: String,
    pub enabled: bool,
    pub endpoint: String,
    pub auth_mode: String,
    pub credential_ref: String,
    pub dm_policy: String,
    pub group_policy: String,
    pub streaming: String,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChannelRuntimeOverview {
    pub registry_path: PathBuf,
    pub delivery_path: PathBuf,
    pub total_count: usize,
    pub enabled_count: usize,
    pub channel_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChannelSyncResult {
    pub registry_path: PathBuf,
    pub total_count: usize,
    pub enabled_count: usize,
    pub channel_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChannelDeliveryRequest {
    pub channel_id: String,
    pub recipient: String,
    pub raw_text: String,
    pub allow_receipt_hashes: bool,
    pub allow_operator_diagnostics: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChannelDeliveryRecord {
    pub delivery_id: String,
    pub channel_id: String,
    pub recipient: String,
    pub submitted_at_unix_ms: u64,
    pub source_class: String,
    pub final_class: String,
    pub allowed: bool,
    pub status: String,
    pub display_text: String,
    pub deny_reason: String,
    pub redactions_applied: Vec<String>,
    pub detected_tokens: Vec<String>,
}

pub fn channel_registry_path(root: &Path) -> PathBuf {
    root.join(DEFAULT_CHANNEL_REGISTRY_PATH)
}

pub fn channel_delivery_path(root: &Path) -> PathBuf {
    root.join(DEFAULT_CHANNEL_DELIVERY_DIR)
}

pub fn ensure_channel_runtime_scaffold(root: &Path) -> LoomResult<PathBuf> {
    let registry_path = channel_registry_path(root);
    if let Some(parent) = registry_path.parent() {
        fs::create_dir_all(parent).map_err(io_err)?;
    }
    fs::create_dir_all(channel_delivery_path(root)).map_err(io_err)?;
    if !registry_path.exists() {
        sync_channel_registry(root)?;
    }
    Ok(registry_path)
}

pub fn sync_channel_registry(root: &Path) -> LoomResult<ChannelSyncResult> {
    let manifest = load_onboard_manifest(root)?;
    let records = channel_records_from_manifest(&manifest);
    persist_channel_registry(root, &records)?;
    Ok(ChannelSyncResult {
        registry_path: channel_registry_path(root),
        total_count: records.len(),
        enabled_count: records.iter().filter(|record| record.enabled).count(),
        channel_ids: records.iter().map(|record| record.channel_id.clone()).collect(),
    })
}

pub fn load_channels(root: &Path) -> LoomResult<Vec<ChannelRecord>> {
    ensure_channel_runtime_scaffold(root)?;
    let raw = fs::read_to_string(channel_registry_path(root)).map_err(io_err)?;
    parse_channel_registry(&raw)
}

pub fn channel_overview(root: &Path) -> LoomResult<ChannelRuntimeOverview> {
    let records = load_channels(root)?;
    Ok(ChannelRuntimeOverview {
        registry_path: channel_registry_path(root),
        delivery_path: channel_delivery_path(root),
        total_count: records.len(),
        enabled_count: records.iter().filter(|record| record.enabled).count(),
        channel_ids: records.iter().map(|record| record.channel_id.clone()).collect(),
    })
}

pub fn enqueue_channel_delivery(root: &Path, request: &ChannelDeliveryRequest) -> LoomResult<ChannelDeliveryRecord> {
    let channel_id = request.channel_id.trim();
    if channel_id.is_empty() {
        return Err("channel_id is required".to_string());
    }
    let recipient = request.recipient.trim();
    if recipient.is_empty() {
        return Err("recipient is required".to_string());
    }

    let channels = load_channels(root)?;
    let channel = channels
        .iter()
        .find(|record| record.channel_id == channel_id)
        .ok_or_else(|| format!("channel '{}' was not found", channel_id))?;

    let submitted_at_unix_ms = now_unix_ms();
    let delivery_id = format!("delivery-{}", unique_token());
    let mut delivery = if !channel.enabled {
        ChannelDeliveryRecord {
            delivery_id,
            channel_id: channel.channel_id.clone(),
            recipient: recipient.to_string(),
            submitted_at_unix_ms,
            source_class: "channel_disabled".to_string(),
            final_class: "blocked".to_string(),
            allowed: false,
            status: "blocked".to_string(),
            display_text: String::new(),
            deny_reason: format!("channel '{}' is disabled", channel.channel_id),
            redactions_applied: Vec::new(),
            detected_tokens: Vec::new(),
        }
    } else {
        let guarded = guard_user_visible_output(
            &request.raw_text,
            &OutputGuardPolicy {
                allow_receipt_hashes: request.allow_receipt_hashes,
                allow_operator_diagnostics: request.allow_operator_diagnostics,
            },
        )?;
        ChannelDeliveryRecord {
            delivery_id,
            channel_id: channel.channel_id.clone(),
            recipient: recipient.to_string(),
            submitted_at_unix_ms,
            source_class: guarded.source_class,
            final_class: guarded.final_class,
            allowed: guarded.allowed,
            status: if guarded.allowed {
                "queued".to_string()
            } else {
                "blocked".to_string()
            },
            display_text: guarded.display_text,
            deny_reason: guarded.deny_reason.unwrap_or_default(),
            redactions_applied: guarded.redactions_applied,
            detected_tokens: guarded.detected_tokens,
        }
    };

    persist_delivery_record(root, &delivery)?;
    delivery.display_text = delivery.display_text.trim().to_string();
    Ok(delivery)
}

pub fn list_channel_deliveries(root: &Path, limit: usize) -> LoomResult<Vec<ChannelDeliveryRecord>> {
    ensure_channel_runtime_scaffold(root)?;
    let mut records = Vec::new();
    for entry in fs::read_dir(channel_delivery_path(root)).map_err(io_err)? {
        let entry = entry.map_err(io_err)?;
        if !entry.file_type().map_err(io_err)?.is_file() {
            continue;
        }
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        let raw = fs::read_to_string(&path).map_err(io_err)?;
        records.push(parse_delivery_record(&raw)?);
    }
    records.sort_by(|left, right| {
        right
            .submitted_at_unix_ms
            .cmp(&left.submitted_at_unix_ms)
            .then_with(|| right.delivery_id.cmp(&left.delivery_id))
    });
    if limit > 0 && records.len() > limit {
        records.truncate(limit);
    }
    Ok(records)
}

pub fn render_channel_overview_human(summary: &ChannelRuntimeOverview) -> String {
    format!(
        "registry_path:   {}\ndelivery_path:   {}\ntotal_count:     {}\nenabled_count:   {}\nchannels:        {}\n",
        summary.registry_path.display(),
        summary.delivery_path.display(),
        summary.total_count,
        summary.enabled_count,
        if summary.channel_ids.is_empty() {
            "(none)".to_string()
        } else {
            summary.channel_ids.join(",")
        }
    )
}

pub fn render_channel_overview_json(summary: &ChannelRuntimeOverview) -> String {
    serde_json::to_string_pretty(&json!({
        "registry_path": summary.registry_path.display().to_string(),
        "delivery_path": summary.delivery_path.display().to_string(),
        "total_count": summary.total_count,
        "enabled_count": summary.enabled_count,
        "channel_ids": summary.channel_ids,
    }))
    .unwrap_or_else(|_| "{}".to_string())
        + "\n"
}

pub fn render_channel_sync_human(result: &ChannelSyncResult) -> String {
    format!(
        "registry_path:   {}\ntotal_count:     {}\nenabled_count:   {}\nchannels:        {}\n",
        result.registry_path.display(),
        result.total_count,
        result.enabled_count,
        if result.channel_ids.is_empty() {
            "(none)".to_string()
        } else {
            result.channel_ids.join(",")
        }
    )
}

pub fn render_channel_sync_json(result: &ChannelSyncResult) -> String {
    serde_json::to_string_pretty(&json!({
        "registry_path": result.registry_path.display().to_string(),
        "total_count": result.total_count,
        "enabled_count": result.enabled_count,
        "channel_ids": result.channel_ids,
    }))
    .unwrap_or_else(|_| "{}".to_string())
        + "\n"
}

pub fn render_channel_delivery_human(record: &ChannelDeliveryRecord) -> String {
    format!(
        "delivery_id:        {}\nchannel_id:         {}\nrecipient:          {}\nsubmitted_at:       {}\nallowed:            {}\nstatus:             {}\nsource_class:       {}\nfinal_class:        {}\ndeny_reason:        {}\nredactions:         {}\ndetected_tokens:    {}\noutput:\n{}\n",
        record.delivery_id,
        record.channel_id,
        record.recipient,
        record.submitted_at_unix_ms,
        record.allowed,
        record.status,
        record.source_class,
        record.final_class,
        if record.deny_reason.is_empty() { "(none)" } else { &record.deny_reason },
        if record.redactions_applied.is_empty() {
            "(none)".to_string()
        } else {
            record.redactions_applied.join(",")
        },
        if record.detected_tokens.is_empty() {
            "(none)".to_string()
        } else {
            record.detected_tokens.join(",")
        },
        if record.display_text.is_empty() {
            "(empty)".to_string()
        } else {
            record.display_text.clone()
        }
    )
}

pub fn render_channel_delivery_json(record: &ChannelDeliveryRecord) -> String {
    serde_json::to_string_pretty(&delivery_record_json(record))
        .unwrap_or_else(|_| "{}".to_string())
        + "\n"
}

pub fn render_channel_delivery_list_human(records: &[ChannelDeliveryRecord]) -> String {
    if records.is_empty() {
        return "delivery_count:    0\n".to_string();
    }
    let mut rendered = format!("delivery_count:    {}\n", records.len());
    for record in records {
        rendered.push_str(&format!(
            "\n- {} channel={} recipient={} status={} class={} submitted_at={}\n",
            record.delivery_id,
            record.channel_id,
            record.recipient,
            record.status,
            record.final_class,
            record.submitted_at_unix_ms,
        ));
    }
    rendered
}

pub fn render_channel_delivery_list_json(records: &[ChannelDeliveryRecord]) -> String {
    serde_json::to_string_pretty(&records.iter().map(delivery_record_json).collect::<Vec<_>>())
        .unwrap_or_else(|_| "[]".to_string())
        + "\n"
}

fn channel_records_from_manifest(manifest: &OnboardManifest) -> Vec<ChannelRecord> {
    vec![
        ChannelRecord {
            channel_id: "web_api".to_string(),
            kind: "web_api".to_string(),
            enabled: true,
            endpoint: format!(
                "http://{}:{}",
                bind_host_for(&manifest.gateway_bind),
                manifest.gateway_port
            ),
            auth_mode: manifest.gateway_auth_mode.clone(),
            credential_ref: manifest.gateway_token_env.clone(),
            dm_policy: manifest.session_dm_scope.clone(),
            group_policy: String::new(),
            streaming: "sync".to_string(),
            note: format!("gateway={} remote={}", manifest.gateway_bind, manifest.remote_mode),
        },
        ChannelRecord {
            channel_id: "telegram".to_string(),
            kind: "telegram".to_string(),
            enabled: manifest.telegram_enabled,
            endpoint: "telegram://bot".to_string(),
            auth_mode: "env_token".to_string(),
            credential_ref: manifest.telegram_token_env.clone(),
            dm_policy: manifest.telegram_dm_policy.clone(),
            group_policy: manifest.telegram_group_policy.clone(),
            streaming: manifest.telegram_streaming.clone(),
            note: format!("dm={} group={}", manifest.telegram_dm_policy, manifest.telegram_group_policy),
        },
    ]
}

fn parse_channel_registry(raw: &str) -> LoomResult<Vec<ChannelRecord>> {
    let value: Value = serde_json::from_str(raw)
        .map_err(|error| format!("invalid channel registry json: {error}"))?;
    let channels = value
        .get("channels")
        .and_then(Value::as_array)
        .ok_or_else(|| "channel registry must define a channels array".to_string())?;
    let mut records = Vec::with_capacity(channels.len());
    for channel in channels {
        records.push(parse_channel_record(channel)?);
    }
    Ok(records)
}

fn parse_channel_record(value: &Value) -> LoomResult<ChannelRecord> {
    Ok(ChannelRecord {
        channel_id: value_string(value.get("channel_id"), "channel_id")?,
        kind: value_string(value.get("kind"), "kind")?,
        enabled: value.get("enabled").and_then(Value::as_bool).unwrap_or(true),
        endpoint: value_string_or(value.get("endpoint"), ""),
        auth_mode: value_string_or(value.get("auth_mode"), "none"),
        credential_ref: value_string_or(value.get("credential_ref"), ""),
        dm_policy: value_string_or(value.get("dm_policy"), ""),
        group_policy: value_string_or(value.get("group_policy"), ""),
        streaming: value_string_or(value.get("streaming"), ""),
        note: value_string_or(value.get("note"), ""),
    })
}

fn persist_channel_registry(root: &Path, records: &[ChannelRecord]) -> LoomResult<()> {
    let registry_path = channel_registry_path(root);
    if let Some(parent) = registry_path.parent() {
        fs::create_dir_all(parent).map_err(io_err)?;
    }
    let value = json!({
        "channels": records.iter().map(channel_record_json).collect::<Vec<_>>(),
    });
    let mut rendered = serde_json::to_string_pretty(&value).map_err(|error| error.to_string())?;
    rendered.push('\n');
    fs::write(registry_path, rendered).map_err(io_err)
}

fn persist_delivery_record(root: &Path, record: &ChannelDeliveryRecord) -> LoomResult<()> {
    let file_name = format!(
        "{}-{}.json",
        record.submitted_at_unix_ms,
        safe_file_token(&record.delivery_id)
    );
    let path = channel_delivery_path(root).join(file_name);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(io_err)?;
    }
    let mut rendered = serde_json::to_string_pretty(&delivery_record_json(record))
        .map_err(|error| error.to_string())?;
    rendered.push('\n');
    fs::write(path, rendered).map_err(io_err)
}

fn parse_delivery_record(raw: &str) -> LoomResult<ChannelDeliveryRecord> {
    let value: Value = serde_json::from_str(raw)
        .map_err(|error| format!("invalid channel delivery json: {error}"))?;
    Ok(ChannelDeliveryRecord {
        delivery_id: value_string(value.get("delivery_id"), "delivery_id")?,
        channel_id: value_string(value.get("channel_id"), "channel_id")?,
        recipient: value_string(value.get("recipient"), "recipient")?,
        submitted_at_unix_ms: value_u64(value.get("submitted_at_unix_ms")).unwrap_or(0),
        source_class: value_string_or(value.get("source_class"), "user_visible"),
        final_class: value_string_or(value.get("final_class"), "user_visible"),
        allowed: value.get("allowed").and_then(Value::as_bool).unwrap_or(false),
        status: value_string_or(value.get("status"), "queued"),
        display_text: value_string_or(value.get("display_text"), ""),
        deny_reason: value_string_or(value.get("deny_reason"), ""),
        redactions_applied: value_array_strings(value.get("redactions_applied")),
        detected_tokens: value_array_strings(value.get("detected_tokens")),
    })
}

fn channel_record_json(record: &ChannelRecord) -> Value {
    json!({
        "channel_id": record.channel_id,
        "kind": record.kind,
        "enabled": record.enabled,
        "endpoint": record.endpoint,
        "auth_mode": record.auth_mode,
        "credential_ref": record.credential_ref,
        "dm_policy": record.dm_policy,
        "group_policy": record.group_policy,
        "streaming": record.streaming,
        "note": record.note,
    })
}

fn delivery_record_json(record: &ChannelDeliveryRecord) -> Value {
    json!({
        "delivery_id": record.delivery_id,
        "channel_id": record.channel_id,
        "recipient": record.recipient,
        "submitted_at_unix_ms": record.submitted_at_unix_ms,
        "source_class": record.source_class,
        "final_class": record.final_class,
        "allowed": record.allowed,
        "status": record.status,
        "display_text": record.display_text,
        "deny_reason": record.deny_reason,
        "redactions_applied": record.redactions_applied,
        "detected_tokens": record.detected_tokens,
    })
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

fn value_array_strings(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|entries| {
            entries
                .iter()
                .filter_map(Value::as_str)
                .map(|entry| entry.to_string())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn value_u64(value: Option<&Value>) -> Option<u64> {
    value.and_then(Value::as_u64)
}

fn safe_file_token(input: &str) -> String {
    input
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch.to_ascii_lowercase() } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

fn unique_token() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
        .to_string()
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn io_err(error: std::io::Error) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{init_workspace, onboarding::write_onboard_manifest};

    fn temp_path(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("{}-{}", label, unique));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create temp path");
        path
    }

    #[test]
    fn scaffold_and_sync_register_channels_from_onboard_manifest() {
        let root = temp_path("loom-channel-scaffold");
        init_workspace(&root, "embedded", Some("/tmp/meridian-kernel"), "org_demo")
            .expect("init workspace");

        let mut manifest = load_onboard_manifest(&root).expect("load onboard manifest");
        manifest.telegram_enabled = true;
        write_onboard_manifest(&root, &manifest).expect("write manifest");

        let result = sync_channel_registry(&root).expect("sync channel registry");
        assert_eq!(result.total_count, 2);
        assert_eq!(result.enabled_count, 2);
        let records = load_channels(&root).expect("load channels");
        assert_eq!(records[0].channel_id, "web_api");
        assert_eq!(records[1].channel_id, "telegram");
        assert!(records[1].enabled);
    }

    #[test]
    fn enqueue_blocks_internal_tokens() {
        let root = temp_path("loom-channel-blocked");
        init_workspace(&root, "embedded", Some("/tmp/meridian-kernel"), "org_demo")
            .expect("init workspace");
        ensure_channel_runtime_scaffold(&root).expect("channel scaffold");

        let record = enqueue_channel_delivery(
            &root,
            &ChannelDeliveryRequest {
                channel_id: "web_api".to_string(),
                recipient: "founder".to_string(),
                raw_text: "SLEEP".to_string(),
                allow_receipt_hashes: false,
                allow_operator_diagnostics: false,
            },
        )
        .expect("enqueue delivery");
        assert!(!record.allowed);
        assert_eq!(record.status, "blocked");
        assert_eq!(record.final_class, "blocked");
    }

    #[test]
    fn enqueue_compacts_user_safe_output_and_lists_history() {
        let root = temp_path("loom-channel-history");
        init_workspace(&root, "embedded", Some("/tmp/meridian-kernel"), "org_demo")
            .expect("init workspace");
        ensure_channel_runtime_scaffold(&root).expect("channel scaffold");

        let record = enqueue_channel_delivery(
            &root,
            &ChannelDeliveryRequest {
                channel_id: "web_api".to_string(),
                recipient: "founder".to_string(),
                raw_text: "[✅ FINAL ANSWER]\nMeridian is live\n[🛡️ PoGE PROTOCOL] Cryptographic Audit Root Settled: 0xabc123".to_string(),
                allow_receipt_hashes: true,
                allow_operator_diagnostics: false,
            },
        )
        .expect("enqueue delivery");
        assert!(record.allowed);
        assert_eq!(record.status, "queued");
        assert!(record.display_text.contains("Meridian is live"));
        assert!(record.display_text.contains("[PoGE Receipt] 0xabc123"));

        let history = list_channel_deliveries(&root, 10).expect("list deliveries");
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].delivery_id, record.delivery_id);
    }
}
