use std::io::IsTerminal;

use crate::*;
use loom_core::channels::{
    channel_overview, enqueue_channel_delivery, ingest_channel_message, list_channel_deliveries, list_channel_ingress,
    render_channel_delivery_human, render_channel_delivery_json, render_channel_delivery_list_human,
    render_channel_delivery_list_json, render_channel_ingress_human, render_channel_ingress_json,
    render_channel_ingress_list_human, render_channel_ingress_list_json, render_channel_overview_human,
    render_channel_overview_json, render_channel_sync_human, render_channel_sync_json, sync_channel_registry,
    ChannelDeliveryRequest, ChannelIngressRequest,
};

pub(crate) fn handle_channel(args: &[String]) -> LoomResult<()> {
    match args.first().map(String::as_str) {
        Some("status") => handle_channel_status(&args[1..]),
        Some("sync") => handle_channel_sync(&args[1..]),
        Some("send") => handle_channel_send(&args[1..]),
        Some("deliveries") => handle_channel_deliveries(&args[1..]),
        Some("ingest") => handle_channel_ingest(&args[1..]),
        Some("inbox") => handle_channel_inbox(&args[1..]),
        _ => Err("channel supports 'status', 'sync', 'send', 'deliveries', 'ingest', and 'inbox'".to_string()),
    }
}

fn handle_channel_status(args: &[String]) -> LoomResult<()> {
    let root = root_from(take_value(args, "--root").as_deref())?;
    let format = take_value(args, "--format").unwrap_or_else(|| {
        if std::io::stdout().is_terminal() {
            "human".to_string()
        } else {
            "json".to_string()
        }
    });
    let summary = channel_overview(&root)?;
    match format.as_str() {
        "human" => {
            print_startup_banner();
            print_human(&render_channel_overview_human(&summary));
        }
        _ => print!("{}", render_channel_overview_json(&summary)),
    }
    Ok(())
}

fn handle_channel_sync(args: &[String]) -> LoomResult<()> {
    let root = root_from(take_value(args, "--root").as_deref())?;
    let format = take_value(args, "--format").unwrap_or_else(|| {
        if std::io::stdout().is_terminal() {
            "human".to_string()
        } else {
            "json".to_string()
        }
    });
    let summary = sync_channel_registry(&root)?;
    match format.as_str() {
        "human" => {
            print_startup_banner();
            print_human(&render_channel_sync_human(&summary));
        }
        _ => print!("{}", render_channel_sync_json(&summary)),
    }
    Ok(())
}

fn handle_channel_send(args: &[String]) -> LoomResult<()> {
    let root = root_from(take_value(args, "--root").as_deref())?;
    let channel_id = required_flag(args, "--channel")?;
    let recipient = required_flag(args, "--recipient")?;
    let format = take_value(args, "--format").unwrap_or_else(|| {
        if std::io::stdout().is_terminal() {
            "human".to_string()
        } else {
            "json".to_string()
        }
    });
    let text = if let Some(path) = take_value(args, "--file") {
        std::fs::read_to_string(&path).map_err(|error| format!("failed to read {}: {}", path, error))?
    } else {
        required_flag(args, "--text")?
    };
    let record = enqueue_channel_delivery(
        &root,
        &ChannelDeliveryRequest {
            channel_id,
            recipient,
            raw_text: text,
            allow_receipt_hashes: has_flag(args, "--allow-receipt-hashes"),
            allow_operator_diagnostics: has_flag(args, "--allow-operator-diagnostics"),
        },
    )?;
    match format.as_str() {
        "human" => {
            print_startup_banner();
            print_human(&render_channel_delivery_human(&record));
        }
        _ => print!("{}", render_channel_delivery_json(&record)),
    }
    Ok(())
}

fn handle_channel_deliveries(args: &[String]) -> LoomResult<()> {
    let root = root_from(take_value(args, "--root").as_deref())?;
    let limit = take_value(args, "--limit")
        .and_then(|raw| raw.parse::<usize>().ok())
        .unwrap_or(20);
    let format = take_value(args, "--format").unwrap_or_else(|| {
        if std::io::stdout().is_terminal() {
            "human".to_string()
        } else {
            "json".to_string()
        }
    });
    let records = list_channel_deliveries(&root, limit)?;
    match format.as_str() {
        "human" => {
            print_startup_banner();
            print_human(&render_channel_delivery_list_human(&records));
        }
        _ => print!("{}", render_channel_delivery_list_json(&records)),
    }
    Ok(())
}

fn handle_channel_ingest(args: &[String]) -> LoomResult<()> {
    let root = root_from(take_value(args, "--root").as_deref())?;
    let channel_id = required_flag(args, "--channel")?;
    let peer_id = required_flag(args, "--peer")?;
    let format = take_value(args, "--format").unwrap_or_else(|| {
        if std::io::stdout().is_terminal() {
            "human".to_string()
        } else {
            "json".to_string()
        }
    });
    let text = if let Some(path) = take_value(args, "--file") {
        std::fs::read_to_string(&path).map_err(|error| format!("failed to read {}: {}", path, error))?
    } else {
        required_flag(args, "--text")?
    };
    let record = ingest_channel_message(
        &root,
        &ChannelIngressRequest {
            channel_id,
            peer_id,
            thread_id: take_value(args, "--thread"),
            text,
            agent_override: take_value(args, "--agent-id"),
        },
    )?;
    match format.as_str() {
        "human" => {
            print_startup_banner();
            print_human(&render_channel_ingress_human(&record));
        }
        _ => print!("{}", render_channel_ingress_json(&record)),
    }
    Ok(())
}

fn handle_channel_inbox(args: &[String]) -> LoomResult<()> {
    let root = root_from(take_value(args, "--root").as_deref())?;
    let limit = take_value(args, "--limit")
        .and_then(|raw| raw.parse::<usize>().ok())
        .unwrap_or(20);
    let format = take_value(args, "--format").unwrap_or_else(|| {
        if std::io::stdout().is_terminal() {
            "human".to_string()
        } else {
            "json".to_string()
        }
    });
    let records = list_channel_ingress(&root, limit)?;
    match format.as_str() {
        "human" => {
            print_startup_banner();
            print_human(&render_channel_ingress_list_human(&records));
        }
        _ => print!("{}", render_channel_ingress_list_json(&records)),
    }
    Ok(())
}
