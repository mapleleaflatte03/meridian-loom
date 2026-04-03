use std::io::IsTerminal;

use crate::*;
use loom_core::memory_service::{
    render_memory_entries_human, render_memory_entries_json, render_memory_receipts_human,
    render_memory_receipts_json, render_memory_service_overview_human,
    render_memory_service_overview_json, MemoryService,
};

pub(crate) fn handle_memory(args: &[String]) -> LoomResult<()> {
    match args.first().map(String::as_str) {
        Some("status") | Some("overview") => handle_memory_overview(&args[1..]),
        Some("search") => handle_memory_search(&args[1..]),
        Some("receipts") => handle_memory_receipts(&args[1..]),
        Some("write") => handle_memory_write(&args[1..]),
        Some("remove") => handle_memory_remove(&args[1..]),
        Some("prune") => handle_memory_prune(&args[1..]),
        _ => Err(
            "memory supports 'status', 'overview', 'search', 'receipts', 'write', 'remove', and 'prune'"
                .to_string(),
        ),
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

fn handle_memory_overview(args: &[String]) -> LoomResult<()> {
    let root = root_from(take_value(args, "--root").as_deref())?;
    let format = output_format(args);
    let summary = MemoryService::with_defaults(&root).overview()?;
    match format.as_str() {
        "human" => {
            print_startup_banner();
            print_human(&render_memory_service_overview_human(&summary));
        }
        _ => print!("{}", render_memory_service_overview_json(&summary)),
    }
    Ok(())
}

fn handle_memory_search(args: &[String]) -> LoomResult<()> {
    let root = root_from(take_value(args, "--root").as_deref())?;
    let format = output_format(args);
    let agent_id = required_flag(args, "--agent-id")?;
    let category = take_value(args, "--category");
    let key_prefix = take_value(args, "--key-prefix");
    let entries = MemoryService::with_defaults(&root).search(
        &agent_id,
        category.as_deref(),
        key_prefix.as_deref(),
    )?;
    match format.as_str() {
        "human" => {
            print_startup_banner();
            print_human(&render_memory_entries_human(&entries));
        }
        _ => print!("{}", render_memory_entries_json(&entries)),
    }
    Ok(())
}

fn handle_memory_receipts(args: &[String]) -> LoomResult<()> {
    let root = root_from(take_value(args, "--root").as_deref())?;
    let format = output_format(args);
    let agent_id = take_value(args, "--agent-id");
    let limit = take_value(args, "--limit")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(20);
    let receipts = MemoryService::with_defaults(&root).list_receipts(limit, agent_id.as_deref())?;
    match format.as_str() {
        "human" => {
            print_startup_banner();
            print_human(&render_memory_receipts_human(&receipts));
        }
        _ => print!("{}", render_memory_receipts_json(&receipts)),
    }
    Ok(())
}

fn handle_memory_write(args: &[String]) -> LoomResult<()> {
    let root = root_from(take_value(args, "--root").as_deref())?;
    let format = output_format(args);
    let agent_id = required_flag(args, "--agent-id")?;
    let category = required_flag(args, "--category")?;
    let key = required_flag(args, "--key")?;
    let content = required_flag(args, "--content")?;
    let source = take_value(args, "--source").unwrap_or_else(|| "operator".to_string());
    let entry =
        MemoryService::with_defaults(&root).write(&agent_id, &category, &key, &content, &source)?;
    match format.as_str() {
        "human" => {
            print_startup_banner();
            print_human(&render_memory_entries_human(&[entry]));
        }
        _ => print!("{}", render_memory_entries_json(&[entry])),
    }
    Ok(())
}

fn handle_memory_remove(args: &[String]) -> LoomResult<()> {
    let root = root_from(take_value(args, "--root").as_deref())?;
    let format = output_format(args);
    let agent_id = required_flag(args, "--agent-id")?;
    let category = required_flag(args, "--category")?;
    let key = required_flag(args, "--key")?;
    let removed = MemoryService::with_defaults(&root).remove(&agent_id, &category, &key)?;
    let payload = serde_json::json!({
        "agent_id": agent_id,
        "category": category,
        "key": key,
        "removed": removed,
    });
    match format.as_str() {
        "human" => {
            print_startup_banner();
            print_human(&format!(
                "agent_id:         {}\ncategory:         {}\nkey:              {}\nremoved:          {}\n",
                payload["agent_id"].as_str().unwrap_or(""),
                payload["category"].as_str().unwrap_or(""),
                payload["key"].as_str().unwrap_or(""),
                payload["removed"].as_bool().unwrap_or(false),
            ));
        }
        _ => print!(
            "{}\n",
            serde_json::to_string_pretty(&payload).map_err(|e| e.to_string())?
        ),
    }
    Ok(())
}

fn handle_memory_prune(args: &[String]) -> LoomResult<()> {
    let root = root_from(take_value(args, "--root").as_deref())?;
    let format = output_format(args);
    let pruned = MemoryService::with_defaults(&root).prune()?;
    let payload = serde_json::json!({ "pruned_entries": pruned });
    match format.as_str() {
        "human" => {
            print_startup_banner();
            print_human(&format!("pruned_entries:   {}\n", pruned));
        }
        _ => print!(
            "{}\n",
            serde_json::to_string_pretty(&payload).map_err(|e| e.to_string())?
        ),
    }
    Ok(())
}
