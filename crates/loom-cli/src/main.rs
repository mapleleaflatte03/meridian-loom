use loom_core::{
    capsule_inspect, contract_show, doctor, health, init_workspace, read_config, render_capsule_human,
    render_contract_human, render_contract_json, render_doctor_human, render_doctor_json, root_from,
    status_human, LoomResult,
};
use loom_shadow::render_shadow_report;
use std::env;
use std::process::ExitCode;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("loom: {}", error);
            ExitCode::FAILURE
        }
    }
}

fn run() -> LoomResult<()> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if args.is_empty() {
        print_help();
        return Ok(());
    }

    match args[0].as_str() {
        "init" => handle_init(&args[1..]),
        "doctor" => handle_doctor(&args[1..]),
        "health" => handle_health(&args[1..]),
        "status" => handle_status(&args[1..]),
        "config" => handle_config(&args[1..]),
        "contract" => handle_contract(&args[1..]),
        "capsule" => handle_capsule(&args[1..]),
        "shadow" => handle_shadow(&args[1..]),
        "-h" | "--help" | "help" => {
            print_help();
            Ok(())
        }
        other => Err(format!("unknown command '{}'", other)),
    }
}

fn handle_init(args: &[String]) -> LoomResult<()> {
    let mode = take_value(args, "--mode").unwrap_or_else(|| "standalone".to_string());
    let kernel_path = take_value(args, "--kernel-path");
    let root = root_from(take_value(args, "--root").as_deref())?;
    let org_id = take_value(args, "--org-id").unwrap_or_else(|| "local_foundry".to_string());
    let config = init_workspace(&root, &mode, kernel_path.as_deref(), &org_id)?;
    println!(
        "initialized Loom scaffold at {}\nmode: {}\norg_id: {}\nstate_dir: {}\nkernel_path: {}",
        root.display(),
        config.mode,
        config.org_id,
        config.state_dir,
        if config.kernel_path.is_empty() {
            "(not set)"
        } else {
            &config.kernel_path
        }
    );
    Ok(())
}

fn handle_doctor(args: &[String]) -> LoomResult<()> {
    let root = root_from(take_value(args, "--root").as_deref())?;
    let format = take_value(args, "--format").unwrap_or_else(|| "json".to_string());
    let checks = doctor(&root)?;
    match format.as_str() {
        "human" => print!("{}", render_doctor_human(&checks)),
        _ => print!("{}", render_doctor_json(&checks)),
    }
    Ok(())
}

fn handle_health(args: &[String]) -> LoomResult<()> {
    let root = root_from(take_value(args, "--root").as_deref())?;
    let format = take_value(args, "--format").unwrap_or_else(|| "json".to_string());
    let (healthy, json) = health(&root)?;
    if format == "human" {
        println!(
            "[{}] loom: {}",
            if healthy { "OK" } else { "WARN" },
            if healthy { "healthy" } else { "degraded" }
        );
    } else {
        print!("{}", json);
    }
    Ok(())
}

fn handle_status(args: &[String]) -> LoomResult<()> {
    let root = root_from(take_value(args, "--root").as_deref())?;
    print!("{}", status_human(&root)?);
    Ok(())
}

fn handle_config(args: &[String]) -> LoomResult<()> {
    if args.first().map(String::as_str) != Some("show") {
        return Err("config only supports 'show' in this scaffold".to_string());
    }
    let root = root_from(take_value(args, "--root").as_deref())?;
    let config = read_config(&root)?;
    println!(
        "[runtime]\nmode = {:?}\nkernel_path = {:?}\norg_id = {:?}\nstate_dir = {:?}\n\n[workers]\npython_path = {:?}\ntypescript_path = {:?}\nwasm_dir = {:?}",
        config.mode,
        config.kernel_path,
        config.org_id,
        config.state_dir,
        config.python_path,
        config.typescript_path,
        config.wasm_dir,
    );
    Ok(())
}

fn handle_contract(args: &[String]) -> LoomResult<()> {
    if args.first().map(String::as_str) != Some("show") {
        return Err("contract only supports 'show' in this scaffold".to_string());
    }
    let root = root_from(take_value(args, "--root").as_deref())?;
    let kernel_path = take_value(args, "--kernel-path");
    let format = take_value(args, "--format").unwrap_or_else(|| "human".to_string());
    let snapshot = contract_show(&root, kernel_path.as_deref())?;
    if format == "json" {
        print!("{}", render_contract_json(&snapshot));
    } else {
        print!("{}", render_contract_human(&snapshot));
    }
    Ok(())
}

fn handle_capsule(args: &[String]) -> LoomResult<()> {
    if args.first().map(String::as_str) != Some("inspect") {
        return Err("capsule only supports 'inspect' in this scaffold".to_string());
    }
    let root = root_from(take_value(args, "--root").as_deref())?;
    let inspection = capsule_inspect(&root)?;
    print!("{}", render_capsule_human(&inspection));
    Ok(())
}

fn handle_shadow(args: &[String]) -> LoomResult<()> {
    if args.first().map(String::as_str) != Some("report") {
        return Err("shadow only supports 'report' in this scaffold".to_string());
    }
    let root = root_from(take_value(args, "--root").as_deref())?;
    print!("{}", render_shadow_report(&root)?);
    Ok(())
}

fn take_value(args: &[String], flag: &str) -> Option<String> {
    args.windows(2)
        .find(|pair| pair[0] == flag)
        .map(|pair| pair[1].clone())
}

fn print_help() {
    println!(
        "Meridian Loom experimental scaffold\n\
         \n\
         Commands:\n\
           loom init --mode <embedded|shadow|standalone> [--kernel-path PATH] [--root PATH] [--org-id ID]\n\
           loom doctor [--root PATH] [--format json|human]\n\
           loom health [--root PATH] [--format json|human]\n\
           loom status [--root PATH]\n\
           loom config show [--root PATH]\n\
           loom contract show [--root PATH] [--kernel-path PATH] [--format human|json]\n\
           loom capsule inspect [--root PATH]\n\
           loom shadow report [--root PATH]\n"
    );
}
