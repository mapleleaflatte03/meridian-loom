use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::*;
use loom_core::agent_runtime::{
    open_agent_session, upsert_agent_runtime_profile, write_agent_memory_snapshot,
    AgentRuntimeProfile,
};
use loom_core::channels::{self, ChannelRecord};
use loom_core::memory_service::MemoryService;
use loom_core::onboarding;
use loom_core::recurring::{self, HeartbeatDeliveryTarget, HeartbeatScheduleRequest};
use loom_core::recurring_executor::dispatch_heartbeat_run;
use loom_core::{init_workspace, read_config};

const DEFAULT_PERSONAL_PROVIDER_PROFILE: &str = "local_ollama";
const DEFAULT_PERSONAL_TOOL_SCOPE: &str = "personal_agent_scope";
const DEFAULT_PERSONAL_ROLE: &str = "manager";
const DEFAULT_PERSONAL_HEARTBEAT_CAPABILITY: &str = "loom.system.info.v1";
const DEFAULT_PERSONAL_HEARTBEAT_SECONDS: u64 = 300;
const DEFAULT_TELEGRAM_TOKEN_ENV: &str = "MERIDIAN_TELEGRAM_BOT_TOKEN";
const PERSONAL_AGENT_TEMPLATE_README: &str =
    include_str!("../../../../templates/personal-agent/README.md");
const PERSONAL_AGENT_TEMPLATE_MEMORY: &str =
    include_str!("../../../../templates/personal-agent/MEMORY.md");
const PERSONAL_AGENT_TEMPLATE_SOUL: &str =
    include_str!("../../../../templates/personal-agent/SOUL.md");

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PersonalAgentConfig {
    pub(crate) name: String,
    pub(crate) slug: String,
    pub(crate) agent_id: String,
    pub(crate) display_name: String,
    pub(crate) role: String,
    pub(crate) purpose: String,
    pub(crate) provider_profile: String,
    pub(crate) tool_scope: String,
    pub(crate) org_id: String,
    pub(crate) loom_root: String,
    pub(crate) kernel_path: String,
    pub(crate) service_http_address: String,
    pub(crate) service_token: String,
    pub(crate) heartbeat_capability: String,
    pub(crate) heartbeat_every_seconds: u64,
    pub(crate) telegram_enabled: bool,
    pub(crate) telegram_chat_id: String,
    pub(crate) telegram_token_env: String,
    pub(crate) webhook_enabled: bool,
    pub(crate) webhook_url: String,
    pub(crate) webhook_header: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PersonalAgentLoopState {
    slug: String,
    agent_id: String,
    pid: u32,
    status: String,
    launched_at_unix_ms: u64,
    updated_at_unix_ms: u64,
    last_run_status: String,
    last_tick_unix_ms: u64,
    heartbeat_id: String,
    log_path: String,
    last_memory_sync_unix_ms: u64,
    memory_entries_recalled: usize,
    memory_entries_updated: usize,
    primary_channel_id: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct PersonalAgentMemorySyncState {
    config_hash: String,
    soul_hash: String,
    memory_hash: String,
    last_sync_unix_ms: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct PersonalAgentMemorySyncResult {
    changed_count: usize,
    recalled_count: usize,
    sync_state: PersonalAgentMemorySyncState,
}

pub(crate) fn handle_new_agent(args: &[String]) -> LoomResult<()> {
    if has_flag(args, "--help") || has_flag(args, "-h") {
        print_new_agent_help();
        return Ok(());
    }

    let root = root_from(take_value(args, "--root").as_deref())?;
    let display_name = required_flag(args, "--name")?;
    let slug = sanitize_personal_slug(&display_name);
    if slug.is_empty() {
        return Err("agent name must produce a non-empty slug".to_string());
    }
    let config_path = personal_agent_config_path(&slug)?;
    if config_path.exists() {
        return Err(format!(
            "personal agent config already exists at {}",
            config_path.display()
        ));
    }

    let requested_kernel_path = take_value(args, "--kernel-path").or_else(default_kernel_path);
    let requested_org_id = take_value(args, "--org-id");
    let runtime_config = ensure_runtime_initialized(
        &root,
        requested_kernel_path.as_deref(),
        requested_org_id.as_deref(),
    )?;
    let kernel_path = kernel_path_for(&root, requested_kernel_path.as_deref())?;
    let org_id = requested_org_id.unwrap_or_else(|| runtime_config.org_id.clone());
    let role = take_value(args, "--role").unwrap_or_else(|| DEFAULT_PERSONAL_ROLE.to_string());
    let purpose = take_value(args, "--purpose")
        .unwrap_or_else(|| format!("Governed personal agent for {}", display_name.trim()));
    let provider_profile = take_value(args, "--provider-profile")
        .unwrap_or_else(|| DEFAULT_PERSONAL_PROVIDER_PROFILE.to_string());
    let tool_scope =
        take_value(args, "--tool-scope").unwrap_or_else(|| DEFAULT_PERSONAL_TOOL_SCOPE.to_string());
    let telegram_chat_id = take_value(args, "--telegram-chat-id").unwrap_or_default();
    let webhook_url = take_value(args, "--webhook-url").unwrap_or_default();
    let webhook_header = take_value(args, "--webhook-header").unwrap_or_default();
    let service_token = format!("loom-agent-{}-{}", slug, chrono_like_timestamp());

    let agent_id = register_kernel_agent(
        &kernel_path,
        &org_id,
        display_name.trim(),
        role.trim(),
        purpose.trim(),
        &provider_profile,
    )?;

    let profile = AgentRuntimeProfile {
        agent_id: agent_id.clone(),
        display_name: display_name.trim().to_string(),
        role: role.trim().to_string(),
        workspace_root: format!("agents/personal/{}/workspace", slug),
        memory_root: format!("agents/personal/{}/memory", slug),
        session_root: format!("agents/personal/{}/sessions", slug),
        provider_profile: provider_profile.clone(),
        tool_scope: tool_scope.clone(),
        heartbeat_policy: "persistent".to_string(),
    };
    let _ = upsert_agent_runtime_profile(&root, &profile)?;

    let config = PersonalAgentConfig {
        name: display_name.trim().to_string(),
        slug: slug.clone(),
        agent_id: agent_id.clone(),
        display_name: display_name.trim().to_string(),
        role: role.trim().to_string(),
        purpose: purpose.trim().to_string(),
        provider_profile,
        tool_scope,
        org_id,
        loom_root: root.display().to_string(),
        kernel_path: kernel_path.display().to_string(),
        service_http_address: runtime_config.service_http_address.clone(),
        service_token,
        heartbeat_capability: DEFAULT_PERSONAL_HEARTBEAT_CAPABILITY.to_string(),
        heartbeat_every_seconds: DEFAULT_PERSONAL_HEARTBEAT_SECONDS,
        telegram_enabled: !telegram_chat_id.trim().is_empty(),
        telegram_chat_id,
        telegram_token_env: DEFAULT_TELEGRAM_TOKEN_ENV.to_string(),
        webhook_enabled: !webhook_url.trim().is_empty(),
        webhook_url,
        webhook_header,
    };
    write_personal_agent_config(&config_path, &config)?;
    write_personal_agent_support_files(&config_path, &config)?;
    seed_personal_agent_memory(&root, &config)?;
    let _ = open_agent_session(&root, &agent_id, Some("personal_agent"))?;

    let format = take_value(args, "--format").unwrap_or_else(|| "human".to_string());
    match format.as_str() {
        "json" => {
            print!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "status": "created",
                    "name": config.name,
                    "slug": config.slug,
                    "agent_id": config.agent_id,
                    "runtime_root": config.loom_root,
                    "kernel_path": config.kernel_path,
                    "config_path": config_path.display().to_string(),
                    "agent_dir": config_path.parent().map(|path| path.display().to_string()).unwrap_or_default(),
                    "next_steps": [
                        format!("loom run-agent {}", config.slug),
                        format!("loom agent runtime --root {} --agent-id {}", config.loom_root, config.agent_id),
                        format!("cat {}/SOUL.md", config_path.parent().map(|path| path.display().to_string()).unwrap_or_default()),
                    ],
                }))
                .map_err(|error| error.to_string())?
            );
            println!();
        }
        _ => {
            print_startup_banner();
            print_human(&format!(
                "Meridian Loom // NEW AGENT\n==========================\nname:         {}\nslug:         {}\nagent_id:     {}\norg_id:       {}\nruntime_root: {}\nkernel_path:  {}\nconfig_path:  {}\nagent_dir:    {}\nstatus:       governed personal agent provisioned\n\nNext\n----\n1. loom run-agent {}\n2. loom agent runtime --root \"{}\" --agent-id {}\n3. loom memory search --root \"{}\" --agent-id {} --category profile\n4. edit \"{}/SOUL.md\" and \"{}/MEMORY.md\"\n",
                config.display_name,
                config.slug,
                config.agent_id,
                config.org_id,
                config.loom_root,
                config.kernel_path,
                config_path.display(),
                config_path.parent().map(|path| path.display().to_string()).unwrap_or_default(),
                config.slug,
                config.loom_root,
                config.agent_id,
                config.loom_root,
                config.agent_id,
                config_path.parent().map(|path| path.display().to_string()).unwrap_or_default(),
                config_path.parent().map(|path| path.display().to_string()).unwrap_or_default(),
            ));
        }
    }
    Ok(())
}

pub(crate) fn handle_run_agent(args: &[String]) -> LoomResult<()> {
    if has_flag(args, "--help") || has_flag(args, "-h") {
        print_run_agent_help();
        return Ok(());
    }

    match args.first().map(String::as_str) {
        Some("status") => return handle_run_agent_status(&args[1..]),
        Some("stop") => return handle_run_agent_stop(&args[1..]),
        _ => {}
    }

    let Some(name_or_slug) = positional_name(args) else {
        return Err("run-agent requires an agent name or slug".to_string());
    };
    let config = load_personal_agent_config(&name_or_slug)?;
    let root = root_from(Some(&config.loom_root))?;
    let loop_log_path = personal_agent_log_path(&root, &config.slug)?;
    let state_path = personal_agent_state_path(&root, &config.slug)?;
    let heartbeat_id = heartbeat_id_for_slug(&config.slug);
    let poll_seconds = take_value(args, "--poll-seconds")
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(15);
    let once = has_flag(args, "--once");
    let foreground = has_flag(args, "--foreground") || has_flag(args, "--loop");

    if !foreground {
        if let Some(state) = load_loop_state(&state_path)? {
            if pid_is_running(state.pid) {
                print_startup_banner();
                print_human(&format!(
                    "Meridian Loom // RUN AGENT\n==========================\nname:         {}\nslug:         {}\nagent_id:     {}\nstatus:       already running\npid:          {}\nheartbeat_id: {}\nlog_path:     {}\nstate_path:   {}\n",
                    config.display_name,
                    config.slug,
                    config.agent_id,
                    state.pid,
                    state.heartbeat_id,
                    state.log_path,
                    state_path.display(),
                ));
                return Ok(());
            }
        }

        if let Some(parent) = loop_log_path.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let stdout = fs::File::create(&loop_log_path).map_err(|error| error.to_string())?;
        let stderr = stdout.try_clone().map_err(|error| error.to_string())?;
        let exe = env::current_exe().map_err(|error| error.to_string())?;
        let mut command = Command::new(exe);
        command
            .arg("run-agent")
            .arg(&config.slug)
            .arg("--foreground")
            .arg("--loop")
            .arg("--poll-seconds")
            .arg(poll_seconds.to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr));
        let child = command.spawn().map_err(|error| error.to_string())?;
        write_loop_state(
            &state_path,
            &PersonalAgentLoopState {
                slug: config.slug.clone(),
                agent_id: config.agent_id.clone(),
                pid: child.id(),
                status: "starting".to_string(),
                launched_at_unix_ms: now_unix_ms(),
                updated_at_unix_ms: now_unix_ms(),
                last_run_status: "booting".to_string(),
                last_tick_unix_ms: 0,
                heartbeat_id: heartbeat_id.clone(),
                log_path: loop_log_path.display().to_string(),
                last_memory_sync_unix_ms: 0,
                memory_entries_recalled: 0,
                memory_entries_updated: 0,
                primary_channel_id: configured_delivery_target(&config)
                    .map(|target| target.channel_id)
                    .unwrap_or_default(),
            },
        )?;
        print_startup_banner();
        print_human(&format!(
            "Meridian Loom // RUN AGENT\n==========================\nname:         {}\nslug:         {}\nagent_id:     {}\nstatus:       background loop started\npid:          {}\nheartbeat_id: {}\nlog_path:     {}\nstate_path:   {}\n\nNext\n----\n1. tail -f \"{}\"\n2. loom recurring runs --root \"{}\" --job-type heartbeat\n3. loom channel deliveries --root \"{}\" --include-archived\n",
            config.display_name,
            config.slug,
            config.agent_id,
            child.id(),
            heartbeat_id,
            loop_log_path.display(),
            state_path.display(),
            loop_log_path.display(),
            root.display(),
            root.display(),
        ));
        return Ok(());
    }

    run_agent_loop(
        &root,
        &config,
        &state_path,
        &loop_log_path,
        &heartbeat_id,
        poll_seconds,
        once,
    )
}

fn run_agent_loop(
    root: &Path,
    config: &PersonalAgentConfig,
    state_path: &Path,
    log_path: &Path,
    heartbeat_id: &str,
    poll_seconds: u64,
    once: bool,
) -> LoomResult<()> {
    ensure_runtime_ready_for_personal_agent(root, config)?;
    sync_personal_agent_delivery_channels(root, config)?;
    let delivery_target = configured_delivery_target(config);
    ensure_personal_agent_heartbeat(root, config, heartbeat_id, delivery_target)?;
    clear_stop_request(root, &config.slug)?;
    let mut memory_sync = sync_personal_agent_memory(root, config)?;
    let _ = open_agent_session(root, &config.agent_id, Some("personal_agent"))?;

    loop {
        let now_ms = now_unix_ms();
        if stop_requested(root, &config.slug)? {
            write_loop_state(
                state_path,
                &PersonalAgentLoopState {
                    slug: config.slug.clone(),
                    agent_id: config.agent_id.clone(),
                    pid: std::process::id(),
                    status: "stopped".to_string(),
                    launched_at_unix_ms: load_loop_state(state_path)?
                        .map(|state| state.launched_at_unix_ms)
                        .unwrap_or(now_ms),
                    updated_at_unix_ms: now_ms,
                    last_run_status: "stop requested by operator".to_string(),
                    last_tick_unix_ms: now_ms,
                    heartbeat_id: heartbeat_id.to_string(),
                    log_path: log_path.display().to_string(),
                    last_memory_sync_unix_ms: memory_sync.sync_state.last_sync_unix_ms,
                    memory_entries_recalled: memory_sync.recalled_count,
                    memory_entries_updated: memory_sync.changed_count,
                    primary_channel_id: configured_delivery_target(config)
                        .map(|target| target.channel_id)
                        .unwrap_or_default(),
                },
            )?;
            clear_stop_request(root, &config.slug)?;
            return Ok(());
        }

        let latest_sync = sync_personal_agent_memory(root, config)?;
        if latest_sync.changed_count > 0 || latest_sync.recalled_count > 0 {
            memory_sync = latest_sync;
        }
        let due = recurring::claim_due_heartbeats(root, now_ms, 8)?;
        let mut last_status = if due.is_empty() {
            format!(
                "idle; recall_entries={} updated_entries={}",
                memory_sync.recalled_count, memory_sync.changed_count
            )
        } else {
            format!("dispatching {} heartbeat(s)", due.len())
        };
        for record in due {
            let run = dispatch_heartbeat_run(root, &record)?;
            memory_sync = recall_personal_agent_memory(root, config)?;
            last_status = format!("{} => {}", record.heartbeat_id, run.status);
        }
        let _ = loom_core::agent_runtime::commit_agent_session(
            root,
            &config.agent_id,
            Some("active"),
            Some(&format!(
                "personal agent loop active; heartbeat_id={} status={}",
                heartbeat_id, last_status
            )),
            Some("personal_agent"),
        );
        write_loop_state(
            state_path,
            &PersonalAgentLoopState {
                slug: config.slug.clone(),
                agent_id: config.agent_id.clone(),
                pid: std::process::id(),
                status: "running".to_string(),
                launched_at_unix_ms: load_loop_state(state_path)?
                    .map(|state| state.launched_at_unix_ms)
                    .unwrap_or(now_ms),
                updated_at_unix_ms: now_ms,
                last_run_status: last_status.clone(),
                last_tick_unix_ms: now_ms,
                heartbeat_id: heartbeat_id.to_string(),
                log_path: log_path.display().to_string(),
                last_memory_sync_unix_ms: memory_sync.sync_state.last_sync_unix_ms,
                memory_entries_recalled: memory_sync.recalled_count,
                memory_entries_updated: memory_sync.changed_count,
                primary_channel_id: configured_delivery_target(config)
                    .map(|target| target.channel_id)
                    .unwrap_or_default(),
            },
        )?;
        if once {
            write_loop_state(
                state_path,
                &PersonalAgentLoopState {
                    slug: config.slug.clone(),
                    agent_id: config.agent_id.clone(),
                    pid: 0,
                    status: "completed_once".to_string(),
                    launched_at_unix_ms: load_loop_state(state_path)?
                        .map(|state| state.launched_at_unix_ms)
                        .unwrap_or(now_ms),
                    updated_at_unix_ms: now_ms,
                    last_run_status: last_status.clone(),
                    last_tick_unix_ms: now_ms,
                    heartbeat_id: heartbeat_id.to_string(),
                    log_path: log_path.display().to_string(),
                    last_memory_sync_unix_ms: memory_sync.sync_state.last_sync_unix_ms,
                    memory_entries_recalled: memory_sync.recalled_count,
                    memory_entries_updated: memory_sync.changed_count,
                    primary_channel_id: configured_delivery_target(config)
                        .map(|target| target.channel_id)
                        .unwrap_or_default(),
                },
            )?;
            return Ok(());
        }
        thread::sleep(Duration::from_secs(poll_seconds.max(5)));
    }
}

fn ensure_runtime_ready_for_personal_agent(
    root: &Path,
    config: &PersonalAgentConfig,
) -> LoomResult<()> {
    let runtime_config = read_config(root)?;
    let profile = AgentRuntimeProfile {
        agent_id: config.agent_id.clone(),
        display_name: config.display_name.clone(),
        role: config.role.clone(),
        workspace_root: format!("agents/personal/{}/workspace", config.slug),
        memory_root: format!("agents/personal/{}/memory", config.slug),
        session_root: format!("agents/personal/{}/sessions", config.slug),
        provider_profile: config.provider_profile.clone(),
        tool_scope: config.tool_scope.clone(),
        heartbeat_policy: "persistent".to_string(),
    };
    let _ = upsert_agent_runtime_profile(root, &profile)?;
    let _ = write_agent_memory_snapshot(
        root,
        &config.agent_id,
        &BTreeMap::from([
            ("personal_agent_slug".to_string(), config.slug.clone()),
            (
                "service_http_address".to_string(),
                runtime_config.service_http_address.clone(),
            ),
            (
                "heartbeat_capability".to_string(),
                config.heartbeat_capability.clone(),
            ),
        ]),
    )?;
    if !runtime_service_status(root, None)?.running {
        let start_args = vec![
            "--root".to_string(),
            root.display().to_string(),
            "--kernel-path".to_string(),
            config.kernel_path.clone(),
            "--http-address".to_string(),
            config.service_http_address.clone(),
            "--service-token".to_string(),
            config.service_token.clone(),
        ];
        crate::commands::service::start_service_with_mode(&start_args)?;
    }
    if !supervisor_daemon_status(root)?.running {
        let daemon_args = vec![
            "--root".to_string(),
            root.display().to_string(),
            "--kernel-path".to_string(),
            config.kernel_path.clone(),
            "--poll-seconds".to_string(),
            "5".to_string(),
        ];
        crate::commands::supervisor::handle_supervisor_daemon_start(&daemon_args)?;
    }
    Ok(())
}

pub(crate) fn sync_personal_agent_delivery_channels(
    root: &Path,
    config: &PersonalAgentConfig,
) -> LoomResult<()> {
    let runtime_config = read_config(root)?;
    let mut manifest = onboarding::load_onboard_manifest(root)?;
    if config.telegram_enabled {
        manifest.telegram_enabled = true;
        manifest.telegram_token_env = config.telegram_token_env.clone();
    }
    onboarding::write_onboard_manifest(root, &manifest)?;
    let _ = channels::sync_channel_registry(root)?;
    if config.webhook_enabled && !config.webhook_url.trim().is_empty() {
        let channel_id = webhook_channel_id(&config.slug);
        channels::upsert_channel_record(
            root,
            &ChannelRecord {
                channel_id,
                kind: "webhook".to_string(),
                enabled: true,
                endpoint: config.webhook_url.clone(),
                auth_mode: if config.webhook_header.trim().is_empty() {
                    "none".to_string()
                } else {
                    "inline_header".to_string()
                },
                credential_ref: config.webhook_header.clone(),
                dm_policy: "per-agent".to_string(),
                group_policy: String::new(),
                streaming: "async".to_string(),
                note: format!(
                    "personal_agent={} gateway={}",
                    config.slug, runtime_config.service_http_address
                ),
            },
        )?;
    } else {
        channels::upsert_channel_record(
            root,
            &ChannelRecord {
                channel_id: webhook_channel_id(&config.slug),
                kind: "webhook".to_string(),
                enabled: false,
                endpoint: config.webhook_url.clone(),
                auth_mode: if config.webhook_header.trim().is_empty() {
                    "none".to_string()
                } else {
                    "inline_header".to_string()
                },
                credential_ref: config.webhook_header.clone(),
                dm_policy: "per-agent".to_string(),
                group_policy: String::new(),
                streaming: "async".to_string(),
                note: format!(
                    "personal_agent={} gateway={} status=disabled",
                    config.slug, runtime_config.service_http_address
                ),
            },
        )?;
    }
    Ok(())
}

fn ensure_personal_agent_heartbeat(
    root: &Path,
    config: &PersonalAgentConfig,
    heartbeat_id: &str,
    delivery_target: Option<HeartbeatDeliveryTarget>,
) -> LoomResult<()> {
    if recurring::heartbeat_summary(root, heartbeat_id).is_ok() {
        return Ok(());
    }
    let request = HeartbeatScheduleRequest {
        heartbeat_id: Some(heartbeat_id.to_string()),
        agent_id: config.agent_id.clone(),
        capability_name: config.heartbeat_capability.clone(),
        schedule_kind: "interval".to_string(),
        schedule_expression: String::new(),
        timezone: "UTC".to_string(),
        every_seconds: config.heartbeat_every_seconds.max(30),
        jitter_seconds: 0,
        not_before_unix_ms: None,
        payload_json: serde_json::json!({
            "message": format!(
                "{} heartbeat on Loom. Emit a governed local status receipt.",
                config.display_name
            )
        })
        .to_string(),
        delivery_target,
        max_attempts: 1,
    };
    let _ = recurring::schedule_heartbeat(root, &request)?;
    Ok(())
}

fn seed_personal_agent_memory(root: &Path, config: &PersonalAgentConfig) -> LoomResult<usize> {
    let service = MemoryService::with_defaults(root);
    let profile_text = format!(
        "name={} role={} purpose={} provider_profile={} tool_scope={} telegram_enabled={} webhook_enabled={}",
        config.display_name,
        config.role,
        config.purpose,
        config.provider_profile,
        config.tool_scope,
        config.telegram_enabled,
        config.webhook_enabled
    );
    let _ = service.write(
        &config.agent_id,
        "profile",
        "personal-agent",
        &profile_text,
        "loom.new-agent",
    )?;
    let _ = service.write(
        &config.agent_id,
        "delivery",
        "channel-plan",
        &format!(
            "telegram_chat_id={} webhook_url={}",
            if config.telegram_chat_id.trim().is_empty() {
                "(none)"
            } else {
                &config.telegram_chat_id
            },
            if config.webhook_url.trim().is_empty() {
                "(none)"
            } else {
                &config.webhook_url
            }
        ),
        "loom.run-agent",
    )?;
    let recalled = service.search(&config.agent_id, Some("profile"), None)?;
    Ok(recalled.len())
}

fn sync_personal_agent_memory(
    root: &Path,
    config: &PersonalAgentConfig,
) -> LoomResult<PersonalAgentMemorySyncResult> {
    let service = MemoryService::with_defaults(root);
    let state_path = personal_agent_memory_sync_state_path(root, &config.slug)?;
    let previous = load_memory_sync_state(&state_path)?;
    let agent_dir = personal_agent_config_path(&config.slug)?
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "personal agent directory was not found".to_string())?;

    let config_body = render_personal_agent_config(config);
    let soul_body = fs::read_to_string(agent_dir.join("SOUL.md")).unwrap_or_default();
    let memory_body = fs::read_to_string(agent_dir.join("MEMORY.md")).unwrap_or_default();

    let current = PersonalAgentMemorySyncState {
        config_hash: fingerprint_text(&config_body),
        soul_hash: fingerprint_text(&soul_body),
        memory_hash: fingerprint_text(&memory_body),
        last_sync_unix_ms: now_unix_ms(),
    };

    let mut changed_count = 0usize;
    if previous.config_hash != current.config_hash {
        let _ = service.write(
            &config.agent_id,
            "profile",
            "agent-config",
            &compact_memory_body(&config_body),
            "loom.personal-agent.sync",
        )?;
        changed_count += 1;
    }
    if previous.soul_hash != current.soul_hash {
        let _ = service.write(
            &config.agent_id,
            "soul",
            "operator-soul",
            &compact_memory_body(&soul_body),
            "loom.personal-agent.sync",
        )?;
        changed_count += 1;
    }
    if previous.memory_hash != current.memory_hash {
        let _ = service.write(
            &config.agent_id,
            "notes",
            "operator-memory",
            &compact_memory_body(&memory_body),
            "loom.personal-agent.sync",
        )?;
        changed_count += 1;
    }

    write_memory_sync_state(&state_path, &current)?;
    let recalled = recall_personal_agent_memory(root, config)?;
    Ok(PersonalAgentMemorySyncResult {
        changed_count,
        recalled_count: recalled.recalled_count,
        sync_state: current,
    })
}

fn recall_personal_agent_memory(
    root: &Path,
    config: &PersonalAgentConfig,
) -> LoomResult<PersonalAgentMemorySyncResult> {
    let service = MemoryService::with_defaults(root);
    let entries = service.search(&config.agent_id, None, None)?;
    let state =
        load_memory_sync_state(&personal_agent_memory_sync_state_path(root, &config.slug)?)?;
    Ok(PersonalAgentMemorySyncResult {
        changed_count: 0,
        recalled_count: entries.len(),
        sync_state: state,
    })
}

pub(crate) fn configured_delivery_target(
    config: &PersonalAgentConfig,
) -> Option<HeartbeatDeliveryTarget> {
    if config.telegram_enabled && !config.telegram_chat_id.trim().is_empty() {
        return Some(HeartbeatDeliveryTarget {
            channel_id: "telegram".to_string(),
            recipient: config.telegram_chat_id.clone(),
            allow_receipt_hashes: true,
            allow_operator_diagnostics: false,
        });
    }
    if config.webhook_enabled && !config.webhook_url.trim().is_empty() {
        return Some(HeartbeatDeliveryTarget {
            channel_id: webhook_channel_id(&config.slug),
            recipient: config.webhook_url.clone(),
            allow_receipt_hashes: true,
            allow_operator_diagnostics: false,
        });
    }
    None
}

fn ensure_runtime_initialized(
    root: &Path,
    requested_kernel_path: Option<&str>,
    requested_org_id: Option<&str>,
) -> LoomResult<loom_core::Config> {
    if root.join("loom.toml").exists() {
        let mut config = read_config(root)?;
        if config.kernel_path.trim().is_empty() {
            let fallback_kernel = default_kernel_path();
            let kernel_path = requested_kernel_path
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .or(fallback_kernel.as_deref())
                .ok_or_else(|| {
                    "kernel path is required to bootstrap a personal agent".to_string()
                })?;
            config.kernel_path = kernel_path.to_string();
            let _ = loom_core::write_config(root, &config)?;
        }
        return Ok(config);
    }

    let fallback_kernel = default_kernel_path();
    let kernel_path = requested_kernel_path
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or(fallback_kernel.as_deref())
        .ok_or_else(|| "kernel path is required to initialize Loom".to_string())?;
    let org_id = requested_org_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("local_foundry");
    init_workspace(root, "embedded", Some(kernel_path), org_id)
}

fn register_kernel_agent(
    kernel_path: &Path,
    org_id: &str,
    display_name: &str,
    role: &str,
    purpose: &str,
    provider_profile: &str,
) -> LoomResult<String> {
    let script = kernel_path.join("kernel/agent_registry.py");
    if !script.exists() {
        return Err(format!("missing {}", script.display()));
    }
    let scopes = format!(
        "governed_local,personal_agent,{},memory_receipts,channel_delivery",
        provider_profile
    );
    let output = Command::new("python3")
        .arg(&script)
        .arg("register")
        .arg("--org_id")
        .arg(org_id)
        .arg("--name")
        .arg(display_name)
        .arg("--role")
        .arg(role)
        .arg("--purpose")
        .arg(purpose)
        .arg("--scopes")
        .arg(scopes)
        .arg("--runtime_binding")
        .arg("loom_native")
        .output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .find_map(|line| line.strip_prefix("Registered agent: "))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "kernel agent registration did not return an agent id".to_string())
}

pub(crate) fn load_personal_agent_config(name_or_slug: &str) -> LoomResult<PersonalAgentConfig> {
    let slug = sanitize_personal_slug(name_or_slug);
    let direct = personal_agent_config_path(&slug)?;
    if direct.exists() {
        return parse_personal_agent_config(&direct);
    }
    let root = personal_agents_config_root()?;
    if !root.exists() {
        return Err(format!(
            "personal agent config was not found for '{}'",
            name_or_slug
        ));
    }
    for entry in fs::read_dir(&root).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path().join("agent.toml");
        if !path.exists() {
            continue;
        }
        let config = parse_personal_agent_config(&path)?;
        if config.slug == slug
            || config.agent_id == name_or_slug.trim()
            || config.name.eq_ignore_ascii_case(name_or_slug.trim())
        {
            return Ok(config);
        }
    }
    Err(format!(
        "personal agent config was not found for '{}'",
        name_or_slug
    ))
}

fn parse_personal_agent_config(path: &Path) -> LoomResult<PersonalAgentConfig> {
    let raw = fs::read_to_string(path).map_err(|error| error.to_string())?;
    let mut values = BTreeMap::new();
    for raw_line in raw.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with('[') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        values.insert(
            key.trim().to_string(),
            value.trim().trim_matches('"').to_string(),
        );
    }
    Ok(PersonalAgentConfig {
        name: required_config_value(&values, "name")?,
        slug: required_config_value(&values, "slug")?,
        agent_id: required_config_value(&values, "agent_id")?,
        display_name: required_config_value(&values, "display_name")?,
        role: required_config_value(&values, "role")?,
        purpose: required_config_value(&values, "purpose")?,
        provider_profile: required_config_value(&values, "provider_profile")?,
        tool_scope: required_config_value(&values, "tool_scope")?,
        org_id: required_config_value(&values, "org_id")?,
        loom_root: required_config_value(&values, "loom_root")?,
        kernel_path: required_config_value(&values, "kernel_path")?,
        service_http_address: required_config_value(&values, "service_http_address")?,
        service_token: required_config_value(&values, "service_token")?,
        heartbeat_capability: required_config_value(&values, "heartbeat_capability")?,
        heartbeat_every_seconds: values
            .get("heartbeat_every_seconds")
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(DEFAULT_PERSONAL_HEARTBEAT_SECONDS),
        telegram_enabled: values
            .get("telegram_enabled")
            .map(|value| value.eq_ignore_ascii_case("true"))
            .unwrap_or(false),
        telegram_chat_id: values.get("telegram_chat_id").cloned().unwrap_or_default(),
        telegram_token_env: values
            .get("telegram_token_env")
            .cloned()
            .unwrap_or_else(|| DEFAULT_TELEGRAM_TOKEN_ENV.to_string()),
        webhook_enabled: values
            .get("webhook_enabled")
            .map(|value| value.eq_ignore_ascii_case("true"))
            .unwrap_or(false),
        webhook_url: values.get("webhook_url").cloned().unwrap_or_default(),
        webhook_header: values.get("webhook_header").cloned().unwrap_or_default(),
    })
}

pub(crate) fn write_personal_agent_config(
    path: &Path,
    config: &PersonalAgentConfig,
) -> LoomResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    fs::write(path, render_personal_agent_config(config)).map_err(|error| error.to_string())
}

fn write_personal_agent_support_files(path: &Path, config: &PersonalAgentConfig) -> LoomResult<()> {
    let Some(agent_dir) = path.parent() else {
        return Ok(());
    };
    fs::create_dir_all(agent_dir).map_err(|error| error.to_string())?;
    write_template_if_missing(
        &agent_dir.join("README.md"),
        &render_personal_agent_template(PERSONAL_AGENT_TEMPLATE_README, config),
    )?;
    write_template_if_missing(
        &agent_dir.join("MEMORY.md"),
        &render_personal_agent_template(PERSONAL_AGENT_TEMPLATE_MEMORY, config),
    )?;
    write_template_if_missing(
        &agent_dir.join("SOUL.md"),
        &render_personal_agent_template(PERSONAL_AGENT_TEMPLATE_SOUL, config),
    )?;
    Ok(())
}

fn render_personal_agent_config(config: &PersonalAgentConfig) -> String {
    format!(
        "[agent]\nname = {}\nslug = {}\nagent_id = {}\ndisplay_name = {}\nrole = {}\npurpose = {}\nprovider_profile = {}\ntool_scope = {}\n\n[runtime]\norg_id = {}\nloom_root = {}\nkernel_path = {}\nservice_http_address = {}\nservice_token = {}\n\n[heartbeat]\nheartbeat_capability = {}\nheartbeat_every_seconds = {}\n\n[telegram]\ntelegram_enabled = {}\ntelegram_chat_id = {}\ntelegram_token_env = {}\n\n[webhook]\nwebhook_enabled = {}\nwebhook_url = {}\nwebhook_header = {}\n",
        json_string(&config.name),
        json_string(&config.slug),
        json_string(&config.agent_id),
        json_string(&config.display_name),
        json_string(&config.role),
        json_string(&config.purpose),
        json_string(&config.provider_profile),
        json_string(&config.tool_scope),
        json_string(&config.org_id),
        json_string(&config.loom_root),
        json_string(&config.kernel_path),
        json_string(&config.service_http_address),
        json_string(&config.service_token),
        json_string(&config.heartbeat_capability),
        config.heartbeat_every_seconds,
        if config.telegram_enabled { "true" } else { "false" },
        json_string(&config.telegram_chat_id),
        json_string(&config.telegram_token_env),
        if config.webhook_enabled { "true" } else { "false" },
        json_string(&config.webhook_url),
        json_string(&config.webhook_header),
    )
}

fn write_template_if_missing(path: &Path, contents: &str) -> LoomResult<()> {
    if path.exists() {
        return Ok(());
    }
    fs::write(path, contents).map_err(|error| error.to_string())
}

fn render_personal_agent_template(template: &str, config: &PersonalAgentConfig) -> String {
    template
        .replace("{{NAME}}", &config.display_name)
        .replace("{{SLUG}}", &config.slug)
        .replace("{{AGENT_ID}}", &config.agent_id)
        .replace("{{ROLE}}", &config.role)
        .replace("{{PURPOSE}}", &config.purpose)
        .replace("{{PROVIDER_PROFILE}}", &config.provider_profile)
        .replace("{{TOOL_SCOPE}}", &config.tool_scope)
        .replace("{{ORG_ID}}", &config.org_id)
        .replace("{{LOOM_ROOT}}", &config.loom_root)
        .replace("{{KERNEL_PATH}}", &config.kernel_path)
        .replace("{{SERVICE_HTTP_ADDRESS}}", &config.service_http_address)
        .replace("{{SERVICE_TOKEN}}", &config.service_token)
        .replace("{{HEARTBEAT_CAPABILITY}}", &config.heartbeat_capability)
        .replace(
            "{{HEARTBEAT_EVERY_SECONDS}}",
            &config.heartbeat_every_seconds.to_string(),
        )
        .replace(
            "{{TELEGRAM_ENABLED}}",
            if config.telegram_enabled {
                "true"
            } else {
                "false"
            },
        )
        .replace("{{TELEGRAM_CHAT_ID}}", &config.telegram_chat_id)
        .replace("{{TELEGRAM_TOKEN_ENV}}", &config.telegram_token_env)
        .replace(
            "{{WEBHOOK_ENABLED}}",
            if config.webhook_enabled {
                "true"
            } else {
                "false"
            },
        )
        .replace("{{WEBHOOK_URL}}", &config.webhook_url)
        .replace("{{WEBHOOK_HEADER}}", &config.webhook_header)
}

fn personal_agents_config_root() -> LoomResult<PathBuf> {
    Ok(config_home()?.join("meridian-loom").join("agents"))
}

pub(crate) fn personal_agent_config_path(slug: &str) -> LoomResult<PathBuf> {
    Ok(personal_agents_config_root()?.join(slug).join("agent.toml"))
}

fn personal_agent_log_path(root: &Path, slug: &str) -> LoomResult<PathBuf> {
    Ok(root
        .join("run")
        .join("personal-agents")
        .join(format!("{}.log", slug)))
}

fn personal_agent_state_path(root: &Path, slug: &str) -> LoomResult<PathBuf> {
    Ok(root
        .join("run")
        .join("personal-agents")
        .join(format!("{}.state.json", slug)))
}

fn personal_agent_stop_request_path(root: &Path, slug: &str) -> LoomResult<PathBuf> {
    Ok(root
        .join("run")
        .join("personal-agents")
        .join(format!("{}.stop", slug)))
}

fn personal_agent_memory_sync_state_path(root: &Path, slug: &str) -> LoomResult<PathBuf> {
    Ok(root
        .join("run")
        .join("personal-agents")
        .join(format!("{}.memory-sync.json", slug)))
}

fn heartbeat_id_for_slug(slug: &str) -> String {
    format!("personal-{}-heartbeat", slug)
}

pub(crate) fn webhook_channel_id(slug: &str) -> String {
    format!("webhook_{}", slug)
}

fn config_home() -> LoomResult<PathBuf> {
    if let Ok(value) = env::var("XDG_CONFIG_HOME") {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return Ok(PathBuf::from(trimmed));
        }
    }
    let home = env::var("HOME").map_err(|_| "HOME is not set".to_string())?;
    Ok(PathBuf::from(home).join(".config"))
}

fn default_kernel_path() -> Option<String> {
    for candidate in ["/opt/meridian-kernel", "/tmp/meridian-kernel"] {
        if Path::new(candidate).exists() {
            return Some(candidate.to_string());
        }
    }
    env::var("MERIDIAN_KERNEL_PATH")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn required_config_value(values: &BTreeMap<String, String>, key: &str) -> LoomResult<String> {
    values
        .get(key)
        .cloned()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("agent config missing {}", key))
}

fn positional_name(args: &[String]) -> Option<String> {
    args.iter()
        .find(|value| !value.starts_with('-'))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn sanitize_personal_slug(input: &str) -> String {
    sanitize_token(input)
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn load_loop_state(path: &Path) -> LoomResult<Option<PersonalAgentLoopState>> {
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(path).map_err(|error| error.to_string())?;
    let value: serde_json::Value = serde_json::from_str(&raw).map_err(|error| error.to_string())?;
    Ok(Some(PersonalAgentLoopState {
        slug: value
            .get("slug")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string(),
        agent_id: value
            .get("agent_id")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string(),
        pid: value
            .get("pid")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or_default() as u32,
        status: value
            .get("status")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string(),
        launched_at_unix_ms: value
            .get("launched_at_unix_ms")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or_default(),
        updated_at_unix_ms: value
            .get("updated_at_unix_ms")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or_default(),
        last_run_status: value
            .get("last_run_status")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string(),
        last_tick_unix_ms: value
            .get("last_tick_unix_ms")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or_default(),
        heartbeat_id: value
            .get("heartbeat_id")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string(),
        log_path: value
            .get("log_path")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string(),
        last_memory_sync_unix_ms: value
            .get("last_memory_sync_unix_ms")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or_default(),
        memory_entries_recalled: value
            .get("memory_entries_recalled")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or_default() as usize,
        memory_entries_updated: value
            .get("memory_entries_updated")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or_default() as usize,
        primary_channel_id: value
            .get("primary_channel_id")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string(),
    }))
}

fn write_loop_state(path: &Path, state: &PersonalAgentLoopState) -> LoomResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let rendered = serde_json::to_string_pretty(&serde_json::json!({
        "slug": state.slug,
        "agent_id": state.agent_id,
        "pid": state.pid,
        "status": state.status,
        "launched_at_unix_ms": state.launched_at_unix_ms,
        "updated_at_unix_ms": state.updated_at_unix_ms,
        "last_run_status": state.last_run_status,
        "last_tick_unix_ms": state.last_tick_unix_ms,
        "heartbeat_id": state.heartbeat_id,
        "log_path": state.log_path,
        "last_memory_sync_unix_ms": state.last_memory_sync_unix_ms,
        "memory_entries_recalled": state.memory_entries_recalled,
        "memory_entries_updated": state.memory_entries_updated,
        "primary_channel_id": state.primary_channel_id,
    }))
    .map_err(|error| error.to_string())?;
    fs::write(path, format!("{}\n", rendered)).map_err(|error| error.to_string())
}

fn load_memory_sync_state(path: &Path) -> LoomResult<PersonalAgentMemorySyncState> {
    if !path.exists() {
        return Ok(PersonalAgentMemorySyncState::default());
    }
    let raw = fs::read_to_string(path).map_err(|error| error.to_string())?;
    let value: serde_json::Value = serde_json::from_str(&raw).map_err(|error| error.to_string())?;
    Ok(PersonalAgentMemorySyncState {
        config_hash: value
            .get("config_hash")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string(),
        soul_hash: value
            .get("soul_hash")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string(),
        memory_hash: value
            .get("memory_hash")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string(),
        last_sync_unix_ms: value
            .get("last_sync_unix_ms")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or_default(),
    })
}

fn write_memory_sync_state(path: &Path, state: &PersonalAgentMemorySyncState) -> LoomResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let rendered = serde_json::to_string_pretty(&serde_json::json!({
        "config_hash": state.config_hash,
        "soul_hash": state.soul_hash,
        "memory_hash": state.memory_hash,
        "last_sync_unix_ms": state.last_sync_unix_ms,
    }))
    .map_err(|error| error.to_string())?;
    fs::write(path, format!("{}\n", rendered)).map_err(|error| error.to_string())
}

fn stop_requested(root: &Path, slug: &str) -> LoomResult<bool> {
    Ok(personal_agent_stop_request_path(root, slug)?.exists())
}

fn clear_stop_request(root: &Path, slug: &str) -> LoomResult<()> {
    let path = personal_agent_stop_request_path(root, slug)?;
    if path.exists() {
        fs::remove_file(path).map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn write_stop_request(root: &Path, slug: &str) -> LoomResult<PathBuf> {
    let path = personal_agent_stop_request_path(root, slug)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    fs::write(&path, b"stop\n").map_err(|error| error.to_string())?;
    Ok(path)
}

fn fingerprint_text(input: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    input.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn compact_memory_body(input: &str) -> String {
    const MAX_BYTES: usize = 12_000;
    let trimmed = input.trim();
    if trimmed.len() <= MAX_BYTES {
        return trimmed.to_string();
    }
    let mut end = MAX_BYTES;
    while !trimmed.is_char_boundary(end) {
        end = end.saturating_sub(1);
    }
    format!(
        "{}\n\n[loom.personal-agent.sync truncated {} bytes]",
        &trimmed[..end],
        trimmed.len().saturating_sub(end)
    )
}

fn pid_is_running(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    Command::new("kill")
        .arg("-0")
        .arg(pid.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn handle_run_agent_status(args: &[String]) -> LoomResult<()> {
    let Some(name_or_slug) = positional_name(args) else {
        return Err("run-agent status requires an agent name or slug".to_string());
    };
    let config = load_personal_agent_config(&name_or_slug)?;
    let root = root_from(Some(&config.loom_root))?;
    let state_path = personal_agent_state_path(&root, &config.slug)?;
    let stop_path = personal_agent_stop_request_path(&root, &config.slug)?;
    let state = load_loop_state(&state_path)?;
    let running = state
        .as_ref()
        .map(|state| pid_is_running(state.pid))
        .unwrap_or(false);
    let normalized_status = state
        .as_ref()
        .map(|state| {
            if running {
                state.status.clone()
            } else if matches!(state.status.as_str(), "running" | "starting") {
                "stopped".to_string()
            } else {
                state.status.clone()
            }
        })
        .unwrap_or_else(|| "not_started".to_string());
    let primary_channel = configured_delivery_target(&config)
        .map(|target| format!("{} -> {}", target.channel_id, target.recipient))
        .unwrap_or_else(|| "none".to_string());
    let summary = serde_json::json!({
        "name": config.display_name,
        "slug": config.slug,
        "agent_id": config.agent_id,
        "running": running,
        "status": normalized_status,
        "pid": state.as_ref().map(|state| state.pid).unwrap_or_default(),
        "heartbeat_id": heartbeat_id_for_slug(&config.slug),
        "last_run_status": state.as_ref().map(|state| state.last_run_status.clone()).unwrap_or_else(|| "not_started".to_string()),
        "last_tick_unix_ms": state.as_ref().map(|state| state.last_tick_unix_ms).unwrap_or_default(),
        "last_memory_sync_unix_ms": state.as_ref().map(|state| state.last_memory_sync_unix_ms).unwrap_or_default(),
        "memory_entries_recalled": state.as_ref().map(|state| state.memory_entries_recalled).unwrap_or_default(),
        "memory_entries_updated": state.as_ref().map(|state| state.memory_entries_updated).unwrap_or_default(),
        "primary_channel": primary_channel,
        "config_path": personal_agent_config_path(&config.slug)?.display().to_string(),
        "state_path": state_path.display().to_string(),
        "stop_request_path": stop_path.display().to_string(),
        "stop_requested": stop_path.exists(),
    });
    let format = take_value(args, "--format").unwrap_or_else(|| "human".to_string());
    match format.as_str() {
        "json" => {
            print!(
                "{}\n",
                serde_json::to_string_pretty(&summary).map_err(|error| error.to_string())?
            );
        }
        _ => {
            print_startup_banner();
            print_human(&format!(
                "Meridian Loom // RUN AGENT STATUS\n=================================\nname:                  {}\nslug:                  {}\nagent_id:              {}\nrunning:               {}\nstatus:                {}\npid:                   {}\nheartbeat_id:          {}\nlast_run_status:       {}\nlast_tick_unix_ms:     {}\nlast_memory_sync_ms:   {}\nmemory_entries_recalled:{}\nmemory_entries_updated:{}\nprimary_channel:       {}\nconfig_path:           {}\nstate_path:            {}\nstop_requested:        {}\n",
                summary["name"].as_str().unwrap_or(""),
                summary["slug"].as_str().unwrap_or(""),
                summary["agent_id"].as_str().unwrap_or(""),
                summary["running"].as_bool().unwrap_or(false),
                summary["status"].as_str().unwrap_or(""),
                summary["pid"].as_u64().unwrap_or_default(),
                summary["heartbeat_id"].as_str().unwrap_or(""),
                summary["last_run_status"].as_str().unwrap_or(""),
                summary["last_tick_unix_ms"].as_u64().unwrap_or_default(),
                summary["last_memory_sync_unix_ms"].as_u64().unwrap_or_default(),
                summary["memory_entries_recalled"].as_u64().unwrap_or_default(),
                summary["memory_entries_updated"].as_u64().unwrap_or_default(),
                summary["primary_channel"].as_str().unwrap_or(""),
                summary["config_path"].as_str().unwrap_or(""),
                summary["state_path"].as_str().unwrap_or(""),
                summary["stop_requested"].as_bool().unwrap_or(false),
            ));
        }
    }
    Ok(())
}

fn handle_run_agent_stop(args: &[String]) -> LoomResult<()> {
    let Some(name_or_slug) = positional_name(args) else {
        return Err("run-agent stop requires an agent name or slug".to_string());
    };
    let config = load_personal_agent_config(&name_or_slug)?;
    let root = root_from(Some(&config.loom_root))?;
    let state_path = personal_agent_state_path(&root, &config.slug)?;
    let state = load_loop_state(&state_path)?;
    let stop_path = write_stop_request(&root, &config.slug)?;
    let running = state
        .as_ref()
        .map(|state| pid_is_running(state.pid))
        .unwrap_or(false);
    let format = take_value(args, "--format").unwrap_or_else(|| "human".to_string());
    let payload = serde_json::json!({
        "status": if running { "stop_requested" } else { "stop_written_no_process" },
        "name": config.display_name,
        "slug": config.slug,
        "agent_id": config.agent_id,
        "pid": state.as_ref().map(|state| state.pid).unwrap_or_default(),
        "stop_request_path": stop_path.display().to_string(),
    });
    match format.as_str() {
        "json" => {
            print!(
                "{}\n",
                serde_json::to_string_pretty(&payload).map_err(|error| error.to_string())?
            );
        }
        _ => {
            print_startup_banner();
            print_human(&format!(
                "Meridian Loom // RUN AGENT STOP\n===============================\nname:              {}\nslug:              {}\nagent_id:          {}\nstatus:            {}\npid:               {}\nstop_request_path: {}\n",
                payload["name"].as_str().unwrap_or(""),
                payload["slug"].as_str().unwrap_or(""),
                payload["agent_id"].as_str().unwrap_or(""),
                payload["status"].as_str().unwrap_or(""),
                payload["pid"].as_u64().unwrap_or_default(),
                payload["stop_request_path"].as_str().unwrap_or(""),
            ));
        }
    }
    Ok(())
}

fn print_new_agent_help() {
    print_human(
        "Meridian Loom // NEW AGENT HELP
==================================
USAGE:
  loom new-agent --name \"My Assistant\" [OPTIONS]

PURPOSE:
  Provision a governed personal agent on Loom, register it in Kernel with
  runtime_binding=loom_native, create a local runtime profile, and write a
  personal agent folder under ~/.config/meridian-loom/agents/<slug>/ with
  agent.toml, README.md, MEMORY.md, and SOUL.md

OPTIONS:
  --name NAME              Required display name for the new agent
  --root PATH              Loom runtime root (defaults to the standard local root)
  --kernel-path PATH       Meridian Kernel path (defaults to /opt/meridian-kernel when present)
  --org-id ORG             Institution/org id to bind
  --role ROLE              Kernel role (default: manager)
  --purpose TEXT           Purpose statement stored in Kernel
  --provider-profile NAME  Loom provider profile (default: local_ollama)
  --tool-scope NAME        Loom tool scope (default: personal_agent_scope)
  --telegram-chat-id ID    Seed Telegram delivery target
  --webhook-url URL        Seed webhook delivery target
  --webhook-header TEXT    Optional inline header for the webhook channel
  --format human|json      Output format
",
    );
}

fn print_run_agent_help() {
    print_human(
        "Meridian Loom // RUN AGENT HELP
==================================
USAGE:
  loom run-agent <name-or-slug> [OPTIONS]

PURPOSE:
  Start a persistent governed personal agent loop. The loop keeps Loom service
  and supervisor ready, syncs SOUL/MEMORY/config into governed memory with
  receipts, claims due heartbeats, dispatches them through the governed
  runtime, and writes loop state under the runtime root.

SUBCOMMANDS:
  status <name-or-slug>     Show loop state, primary channel, and memory sync state
  stop <name-or-slug>       Write a graceful stop request for the running loop

OPTIONS:
  --foreground             Run the loop in the current terminal
  --loop                   Internal flag used by the background daemon mode
  --poll-seconds N         Loop polling interval (default: 15)
  --once                   Run a single loop tick, then exit
",
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_root(label: &str) -> PathBuf {
        let n = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "loom_personal_agent_{}_{}_{}",
            label,
            std::process::id(),
            n
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create temp root");
        dir
    }

    #[test]
    fn personal_agent_slug_is_sanitized() {
        assert_eq!(sanitize_personal_slug("My Assistant"), "my-assistant");
        assert_eq!(sanitize_personal_slug("  Meridian+Bot  "), "meridian-bot");
    }

    #[test]
    fn personal_agent_config_roundtrip_keeps_core_fields() {
        let config = PersonalAgentConfig {
            name: "My Assistant".to_string(),
            slug: "my-assistant".to_string(),
            agent_id: "agent_my_123".to_string(),
            display_name: "My Assistant".to_string(),
            role: "manager".to_string(),
            purpose: "Governed personal agent".to_string(),
            provider_profile: "local_ollama".to_string(),
            tool_scope: "personal_agent_scope".to_string(),
            org_id: "local_foundry".to_string(),
            loom_root: "/tmp/loom".to_string(),
            kernel_path: "/opt/meridian-kernel".to_string(),
            service_http_address: "127.0.0.1:18910".to_string(),
            service_token: "token".to_string(),
            heartbeat_capability: "loom.system.info.v1".to_string(),
            heartbeat_every_seconds: 300,
            telegram_enabled: false,
            telegram_chat_id: String::new(),
            telegram_token_env: DEFAULT_TELEGRAM_TOKEN_ENV.to_string(),
            webhook_enabled: true,
            webhook_url: "https://example.com/hook".to_string(),
            webhook_header: "Authorization: Bearer test".to_string(),
        };
        let rendered = render_personal_agent_config(&config);
        let temp = std::env::temp_dir().join(format!(
            "loom_personal_agent_config_{}_{}.toml",
            std::process::id(),
            chrono_like_timestamp()
        ));
        fs::write(&temp, rendered).expect("write config");
        let parsed = parse_personal_agent_config(&temp).expect("parse config");
        fs::remove_file(&temp).ok();
        assert_eq!(parsed.agent_id, config.agent_id);
        assert_eq!(parsed.webhook_url, config.webhook_url);
        assert_eq!(parsed.provider_profile, config.provider_profile);
    }

    #[test]
    fn personal_agent_memory_sync_tracks_file_changes() {
        let root = temp_root("memory_sync");
        let slug = format!(
            "my-assistant-{}",
            TEST_COUNTER.fetch_add(1, Ordering::SeqCst)
        );
        let config = PersonalAgentConfig {
            name: "My Assistant".to_string(),
            slug: slug.clone(),
            agent_id: "agent_my_123".to_string(),
            display_name: "My Assistant".to_string(),
            role: "manager".to_string(),
            purpose: "Governed personal agent".to_string(),
            provider_profile: "local_ollama".to_string(),
            tool_scope: "personal_agent_scope".to_string(),
            org_id: "local_foundry".to_string(),
            loom_root: root.display().to_string(),
            kernel_path: "/opt/meridian-kernel".to_string(),
            service_http_address: "127.0.0.1:18910".to_string(),
            service_token: "token".to_string(),
            heartbeat_capability: "loom.system.info.v1".to_string(),
            heartbeat_every_seconds: 300,
            telegram_enabled: true,
            telegram_chat_id: "12345".to_string(),
            telegram_token_env: DEFAULT_TELEGRAM_TOKEN_ENV.to_string(),
            webhook_enabled: false,
            webhook_url: String::new(),
            webhook_header: String::new(),
        };
        let config_path = personal_agent_config_path(&slug).expect("config path");
        write_personal_agent_config(&config_path, &config).expect("write config");
        write_personal_agent_support_files(&config_path, &config).expect("support files");

        let first = sync_personal_agent_memory(&root, &config).expect("first sync");
        assert!(first.changed_count >= 3);
        assert!(first.recalled_count >= 3);

        let second = sync_personal_agent_memory(&root, &config).expect("second sync");
        assert_eq!(second.changed_count, 0);

        let memory_file = config_path.parent().expect("agent dir").join("MEMORY.md");
        fs::write(&memory_file, "# changed\nnew durable fact\n").expect("write memory");
        let third = sync_personal_agent_memory(&root, &config).expect("third sync");
        assert!(third.changed_count >= 1);

        fs::remove_dir_all(&root).ok();
        fs::remove_dir_all(config_path.parent().expect("agent dir").to_path_buf()).ok();
    }

    #[test]
    fn compact_memory_body_marks_truncation() {
        let body = "a".repeat(13_000);
        let compacted = compact_memory_body(&body);
        assert!(compacted.contains("truncated"));
        assert!(compacted.len() < body.len());
    }
}
