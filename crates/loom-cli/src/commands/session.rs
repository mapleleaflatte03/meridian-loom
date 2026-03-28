use std::io::IsTerminal;

use crate::*;
use loom_core::session_policy::{
    clear_session_override, get_session_override,
    get_session_send_policy, list_session_overrides, list_session_send_policies,
    render_session_override_human, render_session_override_json,
    render_session_overrides_list_human, render_session_send_policies_list_human,
    render_session_send_policy_human, render_session_send_policy_json, set_session_override,
    set_session_send_policy,
};
use loom_core::session_provenance::{
    find_session_provenance, list_session_provenance, render_session_provenance_human,
    render_session_provenance_json, render_session_provenance_list_human,
    render_session_provenance_list_json, render_session_provenance_overview_human,
    render_session_provenance_overview_json, session_provenance_overview,
};

pub(crate) fn handle_session(args: &[String]) -> LoomResult<()> {
    if args.is_empty() || matches!(args.first().map(String::as_str), Some("help" | "--help" | "-h")) {
        print_session_help();
        return Ok(());
    }
    match args.first().map(String::as_str) {
        Some("status") => handle_session_status(&args[1..]),
        Some("list") => handle_session_list(&args[1..]),
        Some("show") => handle_session_show(&args[1..]),
        Some("override") => handle_session_override(&args[1..]),
        Some("clear-override") => handle_session_clear_override(&args[1..]),
        Some("send-policy") => handle_session_send_policy(&args[1..]),
        Some("overrides") => handle_session_overrides_list(&args[1..]),
        Some("policies") => handle_session_policies_list(&args[1..]),
        _ => Err(
            "session supports: status, list, show, override, clear-override, send-policy, overrides, policies"
                .to_string(),
        ),
    }
}

fn print_session_help() {
    println!(
        "Meridian Loom // SESSION

Manage session provenance, overrides, and send policies.

USAGE: loom session <COMMAND> [OPTIONS]

COMMANDS:
  status                              Show session provenance overview
  list [--limit N]                    List active sessions
  show --session-key KEY              Show session details + override + send policy
  override --session-key KEY          Set provider override for a session
        [--provider-profile PROFILE]
        [--model MODEL]
  clear-override --session-key KEY    Remove provider override
  send-policy --session-key KEY       Set delivery mode for a session
        --mode MODE
        [--channel-target TARGET]
  overrides                           List all active overrides
  policies                            List all active send policies

GLOBAL OPTIONS:
  --root ROOT                         Workspace root path
  --format human|json                 Output format (default: human)"
    );
}

fn handle_session_status(args: &[String]) -> LoomResult<()> {
    let root = root_from(take_value(args, "--root").as_deref())?;
    let format = format_flag(args);
    let overview = session_provenance_overview(&root)?;
    match format.as_str() {
        "human" => {
            print_startup_banner();
            print_human(&render_session_provenance_overview_human(&overview));
        }
        _ => print!("{}", render_session_provenance_overview_json(&overview)),
    }
    Ok(())
}

fn handle_session_list(args: &[String]) -> LoomResult<()> {
    let root = root_from(take_value(args, "--root").as_deref())?;
    let format = format_flag(args);
    let limit = take_value(args, "--limit")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(20);
    let records = list_session_provenance(&root, limit)?;
    match format.as_str() {
        "human" => {
            print_startup_banner();
            print_human(&render_session_provenance_list_human(&records));
        }
        _ => print!("{}", render_session_provenance_list_json(&records)),
    }
    Ok(())
}

fn handle_session_show(args: &[String]) -> LoomResult<()> {
    let root = root_from(take_value(args, "--root").as_deref())?;
    let session_key = required_flag(args, "--session-key")?;
    let format = format_flag(args);
    let record = find_session_provenance(&root, &session_key)?
        .ok_or_else(|| format!("session '{}' was not found", session_key))?;
    let override_rec = get_session_override(&root, &session_key).ok().flatten();
    let send_policy_rec = get_session_send_policy(&root, &session_key).ok().flatten();
    match format.as_str() {
        "human" => {
            print_startup_banner();
            print_human(&render_session_provenance_human(&record));
            if let Some(ov) = &override_rec {
                print_human("\n[session override]\n");
                print_human(&render_session_override_human(ov));
            }
            if let Some(sp) = &send_policy_rec {
                print_human("\n[send policy]\n");
                print_human(&render_session_send_policy_human(sp));
            }
        }
        _ => {
            use serde_json::json;
            let output = json!({
                "provenance": serde_json::from_str::<serde_json::Value>(&render_session_provenance_json(&record)).unwrap_or_default(),
                "override": override_rec.as_ref().map(|ov| serde_json::from_str::<serde_json::Value>(&render_session_override_json(ov)).unwrap_or_default()),
                "send_policy": send_policy_rec.as_ref().map(|sp| serde_json::from_str::<serde_json::Value>(&render_session_send_policy_json(sp)).unwrap_or_default()),
            });
            print!("{}\n", serde_json::to_string_pretty(&output).unwrap_or_default());
        }
    }
    Ok(())
}

fn handle_session_override(args: &[String]) -> LoomResult<()> {
    let root = root_from(take_value(args, "--root").as_deref())?;
    let session_key = required_flag(args, "--session-key")?;
    let provider_profile = take_value(args, "--provider-profile");
    let model = take_value(args, "--model");
    if provider_profile.is_none() && model.is_none() {
        return Err(
            "at least one of --provider-profile or --model is required for session override"
                .to_string(),
        );
    }
    let format = format_flag(args);
    let record = set_session_override(
        &root,
        &session_key,
        provider_profile.as_deref(),
        model.as_deref(),
    )?;
    match format.as_str() {
        "human" => {
            print_startup_banner();
            print_human(&render_session_override_human(&record));
        }
        _ => print!("{}", render_session_override_json(&record)),
    }
    Ok(())
}

fn handle_session_clear_override(args: &[String]) -> LoomResult<()> {
    let root = root_from(take_value(args, "--root").as_deref())?;
    let session_key = required_flag(args, "--session-key")?;
    clear_session_override(&root, &session_key)?;
    if std::io::stdout().is_terminal() {
        print_startup_banner();
        print_human(&format!("override cleared for session '{}'\n", session_key));
    } else {
        print!(
            "{{\"status\":\"cleared\",\"session_key\":{:?}}}\n",
            session_key
        );
    }
    Ok(())
}

fn handle_session_send_policy(args: &[String]) -> LoomResult<()> {
    let root = root_from(take_value(args, "--root").as_deref())?;
    let session_key = required_flag(args, "--session-key")?;
    let mode = required_flag(args, "--mode")?;
    let channel_target = take_value(args, "--channel-target");
    let format = format_flag(args);
    let record = set_session_send_policy(
        &root,
        &session_key,
        &mode,
        channel_target.as_deref(),
    )?;
    match format.as_str() {
        "human" => {
            print_startup_banner();
            print_human(&render_session_send_policy_human(&record));
        }
        _ => print!("{}", render_session_send_policy_json(&record)),
    }
    Ok(())
}

fn handle_session_overrides_list(args: &[String]) -> LoomResult<()> {
    let root = root_from(take_value(args, "--root").as_deref())?;
    let format = format_flag(args);
    let records = list_session_overrides(&root)?;
    match format.as_str() {
        "human" => {
            print_startup_banner();
            print_human(&render_session_overrides_list_human(&records));
        }
        _ => {
            use serde_json::json;
            let out = json!(records.iter().map(|r| serde_json::from_str::<serde_json::Value>(&render_session_override_json(r)).unwrap_or_default()).collect::<Vec<_>>());
            print!("{}\n", serde_json::to_string_pretty(&out).unwrap_or_default());
        }
    }
    Ok(())
}

fn handle_session_policies_list(args: &[String]) -> LoomResult<()> {
    let root = root_from(take_value(args, "--root").as_deref())?;
    let format = format_flag(args);
    let records = list_session_send_policies(&root)?;
    match format.as_str() {
        "human" => {
            print_startup_banner();
            print_human(&render_session_send_policies_list_human(&records));
        }
        _ => {
            use serde_json::json;
            let out = json!(records.iter().map(|r| serde_json::from_str::<serde_json::Value>(&render_session_send_policy_json(r)).unwrap_or_default()).collect::<Vec<_>>());
            print!("{}\n", serde_json::to_string_pretty(&out).unwrap_or_default());
        }
    }
    Ok(())
}

fn format_flag(args: &[String]) -> String {
    take_value(args, "--format").unwrap_or_else(|| {
        if std::io::stdout().is_terminal() {
            "human".to_string()
        } else {
            "json".to_string()
        }
    })
}
