use std::io::IsTerminal;

use crate::*;
use loom_core::agent_runtime::{agent_runtime_summary, render_agent_runtime_human, render_agent_runtime_json};

pub(crate) fn handle_init(args: &[String]) -> LoomResult<()> {
    let mode = take_value(args, "--mode").unwrap_or_else(|| "standalone".to_string());
    let kernel_path = take_value(args, "--kernel-path");
    let root = root_from(take_value(args, "--root").as_deref())?;
    let org_id = take_value(args, "--org-id").unwrap_or_else(|| "local_foundry".to_string());
    let config = init_workspace(&root, &mode, kernel_path.as_deref(), &org_id)?;
    print_human(&format!(
        "Meridian Loom // INIT\n====================\nroot:        {}\nconfig:      {}\nmode:        {}\norg_id:      {}\nstate_dir:   {}\nrun_dir:     {}\nlog_dir:     {}\nartifact_dir: {}\nkernel_path: {}\nstatus:      initialized local-first runtime root\nnext_step:   loom doctor --root {} --format human\n",
        root.display(),
        root.join("loom.toml").display(),
        config.mode,
        config.org_id,
        config.state_dir,
        config.run_dir,
        config.log_dir,
        config.artifact_dir,
        if config.kernel_path.is_empty() { "(not set)" } else { &config.kernel_path },
        root.display()
    ));
    Ok(())
}


pub(crate) fn handle_doctor(args: &[String]) -> LoomResult<()> {
    let root = root_from(take_value(args, "--root").as_deref())?;
    let format = take_value(args, "--format").unwrap_or_else(|| {
        if std::io::stdout().is_terminal() {
            "human".to_string()
        } else {
            "json".to_string()
        }
    });
    let checks = doctor(&root)?;
    match format.as_str() {
        "human" => {
            print_startup_banner();
            print_human(&render_doctor_human(&checks));
        }
        _ => print!("{}", render_doctor_json(&checks)),
    }
    Ok(())
}


pub(crate) fn handle_health(args: &[String]) -> LoomResult<()> {
    let root = root_from(take_value(args, "--root").as_deref())?;
    let format = take_value(args, "--format").unwrap_or_else(|| "json".to_string());
    let (healthy, json) = health(&root)?;
    if format == "human" {
        print_human(&render_health_human(healthy, &json));
    } else {
        print!("{}", json);
    }
    Ok(())
}


pub(crate) fn handle_status(args: &[String]) -> LoomResult<()> {
    let root = root_from(take_value(args, "--root").as_deref())?;
    let base = status_human(&root)?;
    let service = runtime_service_status(&root, take_value(args, "--socket").as_deref())?;
    print_startup_banner();
    print_human_block(&[base, render_runtime_service_human(&service)]);
    Ok(())
}


pub(crate) fn handle_config(args: &[String]) -> LoomResult<()> {
    if args.first().map(String::as_str) != Some("show") {
        return Err("config only supports 'show' in this scaffold".to_string());
    }
    let root = root_from(take_value(args, "--root").as_deref())?;
    let config = read_config(&root)?;
    print_human(&render_config_human(&config, &root));
    Ok(())
}


pub(crate) fn handle_contract(args: &[String]) -> LoomResult<()> {
    match args.first().map(String::as_str) {
        Some("show") => {
            let root = root_from(take_value(args, "--root").as_deref())?;
            let kernel_path = take_value(args, "--kernel-path");
            let format = take_value(args, "--format").unwrap_or_else(|| "human".to_string());
            let snapshot = contract_show(&root, kernel_path.as_deref())?;
            if format == "json" {
                print!("{}", render_contract_json(&snapshot));
            } else {
                print_human(&render_contract_human(&snapshot));
            }
            Ok(())
        }
        Some("verify") => {
            let root = root_from(take_value(args, "--root").as_deref())?;
            let kernel_path = take_value(args, "--kernel-path");
            let agent_id = take_value(args, "--agent-id")
                .unwrap_or_else(|| "agent_tutorial".to_string());
            let org_id = take_value(args, "--org-id");
            let format = take_value(args, "--format").unwrap_or_else(|| "human".to_string());
            let result = contract_verify(
                &root,
                kernel_path.as_deref(),
                &agent_id,
                org_id.as_deref(),
            )?;
            if format == "json" {
                print!("{}", render_contract_verify_json(&result));
            } else {
                print_human(&render_contract_verify_human(&result));
            }
            if result.passed < result.total {
                std::process::exit(1);
            }
            Ok(())
        }
        _ => Err("contract supports 'show' and 'verify'".to_string()),
    }
}


pub(crate) fn handle_capsule(args: &[String]) -> LoomResult<()> {
    if args.first().map(String::as_str) != Some("inspect") {
        return Err("capsule only supports 'inspect' in this scaffold".to_string());
    }
    let root = root_from(take_value(args, "--root").as_deref())?;
    let inspection = capsule_inspect(&root)?;
    print_human(&render_capsule_human(&inspection));
    Ok(())
}


pub(crate) fn handle_agent(args: &[String]) -> LoomResult<()> {
    match args.first().map(String::as_str) {
        Some("resolve") => {
            let root = root_from(take_value(args, "--root").as_deref())?;
            let agent_id = required_flag(args, "--agent-id")?;
            let kernel_path = take_value(args, "--kernel-path");
            let org_id = take_value(args, "--org-id");
            let format = take_value(args, "--format").unwrap_or_else(|| "human".to_string());
            let identity = resolve_agent_identity(&root, kernel_path.as_deref(), &agent_id, org_id.as_deref())?;
            if format == "json" {
                print!("{}", render_identity_json(&identity));
            } else {
                print_human(&render_identity_human(&identity));
            }
            Ok(())
        }
        Some("runtime") => {
            let root = root_from(take_value(args, "--root").as_deref())?;
            let agent_id = required_flag(args, "--agent-id")?;
            let format = take_value(args, "--format").unwrap_or_else(|| "human".to_string());
            let summary = agent_runtime_summary(&root, &agent_id)?;
            if format == "json" {
                print!("{}", render_agent_runtime_json(&summary));
            } else {
                print_human(&render_agent_runtime_human(&summary));
            }
            Ok(())
        }
        _ => Err("agent supports 'resolve' and 'runtime'".to_string()),
    }
}


pub(crate) fn handle_envelope(args: &[String]) -> LoomResult<()> {
    if args.first().map(String::as_str) != Some("build") {
        return Err("envelope only supports 'build' in this scaffold".to_string());
    }
    let root = root_from(take_value(args, "--root").as_deref())?;
    let agent_id = required_flag(args, "--agent-id")?;
    let action_type = required_flag(args, "--action-type")?;
    let resource = required_flag(args, "--resource")?;
    let estimated_cost_usd = parse_f64_flag(args, "--estimated-cost-usd").unwrap_or(0.0);
    let kernel_path = take_value(args, "--kernel-path");
    let org_id = take_value(args, "--org-id");
    let run_id = take_value(args, "--run-id");
    let session_id = take_value(args, "--session-id");
    let format = take_value(args, "--format").unwrap_or_else(|| "human".to_string());

    let envelope = build_action_envelope(
        &root,
        kernel_path.as_deref(),
        &agent_id,
        org_id.as_deref(),
        &action_type,
        &resource,
        estimated_cost_usd,
        run_id.as_deref(),
        session_id.as_deref(),
    )?;
    if format == "json" {
        print!("{}", render_envelope_json(&envelope));
    } else {
        print_human(&render_envelope_human(&envelope));
    }
    Ok(())
}


pub(crate) fn handle_shadow(args: &[String]) -> LoomResult<()> {
    match args.first().map(String::as_str) {
        Some("report") => {
            let root = root_from(take_value(args, "--root").as_deref())?;
            print_human(&render_shadow_report(&root)?);
            Ok(())
        }
        Some("preflight") => {
            let root = root_from(take_value(args, "--root").as_deref())?;
            let agent_id = required_flag(args, "--agent-id")?;
            let action_type = required_flag(args, "--action-type")?;
            let resource = required_flag(args, "--resource")?;
            let estimated_cost_usd = parse_f64_flag(args, "--estimated-cost-usd").unwrap_or(0.0);
            let kernel_path = take_value(args, "--kernel-path");
            let org_id = take_value(args, "--org-id");
            let run_id = take_value(args, "--run-id");
            let session_id = take_value(args, "--session-id");
            let format = take_value(args, "--format").unwrap_or_else(|| "human".to_string());

            let identity = resolve_agent_identity(&root, kernel_path.as_deref(), &agent_id, org_id.as_deref())?;
            let envelope = build_action_envelope(
                &root,
                kernel_path.as_deref(),
                &agent_id,
                org_id.as_deref(),
                &action_type,
                &resource,
                estimated_cost_usd,
                run_id.as_deref(),
                session_id.as_deref(),
            )?;
            let reference =
                evaluate_reference_gates(&root, kernel_path.as_deref(), &identity, &envelope)?;
            let effective_kernel_path = kernel_path_for(&root, kernel_path.as_deref())?;
            let capture =
                capture_preflight(&root, &effective_kernel_path, &identity, &envelope, &reference)?;
            if format == "json" {
                print!("{}", render_preflight_json(&capture));
            } else {
                print_human_block(&[
                    render_identity_human(&identity),
                    render_envelope_human(&envelope),
                    render_preflight_human(&capture),
                ]);
            }
            Ok(())
        }
        Some("decide") => {
            let root = root_from(take_value(args, "--root").as_deref())?;
            let agent_id = required_flag(args, "--agent-id")?;
            let action_type = required_flag(args, "--action-type")?;
            let resource = required_flag(args, "--resource")?;
            let estimated_cost_usd = parse_f64_flag(args, "--estimated-cost-usd").unwrap_or(0.0);
            let kernel_path = take_value(args, "--kernel-path");
            let org_id = take_value(args, "--org-id");
            let run_id = take_value(args, "--run-id");
            let session_id = take_value(args, "--session-id");
            let format = take_value(args, "--format").unwrap_or_else(|| "human".to_string());

            let identity =
                resolve_agent_identity(&root, kernel_path.as_deref(), &agent_id, org_id.as_deref())?;
            let envelope = build_action_envelope(
                &root,
                kernel_path.as_deref(),
                &agent_id,
                org_id.as_deref(),
                &action_type,
                &resource,
                estimated_cost_usd,
                run_id.as_deref(),
                session_id.as_deref(),
            )?;
            let reference =
                evaluate_reference_gates(&root, kernel_path.as_deref(), &identity, &envelope)?;
            let capture = capture_decision(&root, &identity, &envelope, &reference)?;
            if format == "json" {
                print!("{}", render_decision_json(&capture));
            } else {
                print_human(&render_decision_human(&capture));
            }
            Ok(())
        }
        Some("enforce") => {
            let root = root_from(take_value(args, "--root").as_deref())?;
            let agent_id = required_flag(args, "--agent-id")?;
            let action_type = required_flag(args, "--action-type")?;
            let resource = required_flag(args, "--resource")?;
            let estimated_cost_usd = parse_f64_flag(args, "--estimated-cost-usd").unwrap_or(0.0);
            let kernel_path = take_value(args, "--kernel-path");
            let org_id = take_value(args, "--org-id");
            let run_id = take_value(args, "--run-id");
            let session_id = take_value(args, "--session-id");
            let format = take_value(args, "--format").unwrap_or_else(|| "human".to_string());

            let identity =
                resolve_agent_identity(&root, kernel_path.as_deref(), &agent_id, org_id.as_deref())?;
            let envelope = build_action_envelope(
                &root,
                kernel_path.as_deref(),
                &agent_id,
                org_id.as_deref(),
                &action_type,
                &resource,
                estimated_cost_usd,
                run_id.as_deref(),
                session_id.as_deref(),
            )?;
            let reference =
                evaluate_reference_gates(&root, kernel_path.as_deref(), &identity, &envelope)?;
            let capture = capture_decision(&root, &identity, &envelope, &reference)?;
            if format == "json" {
                print!("{}", render_decision_json(&capture));
            } else {
                print_human(&render_decision_human(&capture));
            }
            std::process::exit(decision_exit_code(&capture, 0, 2));
        }
        Some("compare") => {
            let root = root_from(take_value(args, "--root").as_deref())?;
            let config = read_config(&root)?;
            let primary = PathBuf::from(required_flag(args, "--primary")?);
            let shadow = take_value(args, "--shadow")
                .map(PathBuf::from)
                .unwrap_or_else(|| root.join(&config.artifact_dir).join("shadow/events.jsonl"));
            let format = take_value(args, "--format").unwrap_or_else(|| "human".to_string());
            let summary = compare_logs(Some(&root), &primary, &shadow)?;
            if format == "json" {
                print!("{}", render_compare_json(&summary));
            } else {
                print_human(&render_compare_human(&summary));
            }
            Ok(())
        }
        _ => Err("shadow supports 'preflight', 'decide', 'enforce', 'compare', and 'report'".to_string()),
    }
}



pub(crate) fn handle_parity(args: &[String]) -> LoomResult<()> {
    match args.first().map(String::as_str) {
        Some("report") => {
            let root = root_from(take_value(args, "--root").as_deref())?;
            print_human(&render_parity_report(&root)?);
            Ok(())
        }
        _ => Err("parity supports 'report'".to_string()),
    }
}
