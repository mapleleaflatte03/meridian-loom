use std::io::IsTerminal;

use crate::*;
use loom_core::skills::{
    find_skill, load_skills, render_skill_human, render_skill_json, render_skill_list_human,
    render_skill_list_json, render_skill_overview_human, render_skill_overview_json,
    render_skill_sync_human, render_skill_sync_json, skill_overview, sync_skill_registry,
};

pub(crate) fn handle_skill(args: &[String]) -> LoomResult<()> {
    match args.first().map(String::as_str) {
        Some("status") => handle_skill_status(&args[1..]),
        Some("sync") => handle_skill_sync(&args[1..]),
        Some("list") => handle_skill_list(&args[1..]),
        Some("show") => handle_skill_show(&args[1..]),
        _ => Err("skill supports 'status', 'sync', 'list', and 'show'".to_string()),
    }
}

fn handle_skill_status(args: &[String]) -> LoomResult<()> {
    let root = root_from(take_value(args, "--root").as_deref())?;
    let format = take_value(args, "--format").unwrap_or_else(|| {
        if std::io::stdout().is_terminal() {
            "human".to_string()
        } else {
            "json".to_string()
        }
    });
    let summary = skill_overview(&root)?;
    match format.as_str() {
        "human" => {
            print_startup_banner();
            print_human(&render_skill_overview_human(&summary));
        }
        _ => print!("{}", render_skill_overview_json(&summary)),
    }
    Ok(())
}

fn handle_skill_sync(args: &[String]) -> LoomResult<()> {
    let root = root_from(take_value(args, "--root").as_deref())?;
    let format = take_value(args, "--format").unwrap_or_else(|| {
        if std::io::stdout().is_terminal() {
            "human".to_string()
        } else {
            "json".to_string()
        }
    });
    let summary = sync_skill_registry(&root)?;
    match format.as_str() {
        "human" => {
            print_startup_banner();
            print_human(&render_skill_sync_human(&summary));
        }
        _ => print!("{}", render_skill_sync_json(&summary)),
    }
    Ok(())
}

fn handle_skill_list(args: &[String]) -> LoomResult<()> {
    let root = root_from(take_value(args, "--root").as_deref())?;
    let format = take_value(args, "--format").unwrap_or_else(|| {
        if std::io::stdout().is_terminal() {
            "human".to_string()
        } else {
            "json".to_string()
        }
    });
    let records = load_skills(&root)?;
    match format.as_str() {
        "human" => {
            print_startup_banner();
            print_human(&render_skill_list_human(&records));
        }
        _ => print!("{}", render_skill_list_json(&records)),
    }
    Ok(())
}

fn handle_skill_show(args: &[String]) -> LoomResult<()> {
    let root = root_from(take_value(args, "--root").as_deref())?;
    let skill_id = required_flag(args, "--skill-id")?;
    let format = take_value(args, "--format").unwrap_or_else(|| {
        if std::io::stdout().is_terminal() {
            "human".to_string()
        } else {
            "json".to_string()
        }
    });
    let record = find_skill(&root, &skill_id)?;
    match format.as_str() {
        "human" => {
            print_startup_banner();
            print_human(&render_skill_human(&record));
        }
        _ => print!("{}", render_skill_json(&record)),
    }
    Ok(())
}
