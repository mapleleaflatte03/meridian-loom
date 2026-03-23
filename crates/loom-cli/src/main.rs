use loom_core::{
    build_action_envelope, capsule_inspect, contract_show, doctor, health, init_workspace,
    kernel_path_for, read_config, render_capsule_human, render_contract_human,
    render_config_human, render_contract_json, render_doctor_human, render_doctor_json,
    render_envelope_human, render_envelope_json, render_health_human, render_identity_human,
    render_identity_json, resolve_agent_identity, root_from, status_human,
    evaluate_reference_gates, LoomResult,
};
use loom_shadow::{
    capture_decision, capture_preflight, capture_runtime_execution, compare_logs,
    decision_exit_code, enqueue_action, inspect_job, list_jobs, render_compare_human,
    render_compare_json, render_decision_human, render_decision_json,
    render_enqueued_action_human, render_enqueued_action_json, render_job_inspect_human,
    render_job_inspect_json, render_job_list_human, render_job_list_json, render_parity_report,
    render_preflight_human, render_preflight_json, render_runtime_execution_human,
    render_runtime_execution_json, render_supervisor_daemon_human,
    render_supervisor_daemon_json, render_shadow_report, render_supervisor_run_human,
    render_supervisor_run_json, render_supervisor_status_human, render_supervisor_status_json,
    render_supervisor_watch_human, render_supervisor_watch_json, run_supervisor,
    run_supervisor_daemon_loop, request_supervisor_daemon_stop, supervisor_daemon_status,
    supervisor_status, watch_supervisor,
};
use std::env;
use std::io::{self, IsTerminal};
use std::path::PathBuf;
use std::process::{Command, ExitCode, Stdio};

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
        "job" => handle_job(&args[1..]),
        "agent" => handle_agent(&args[1..]),
        "envelope" => handle_envelope(&args[1..]),
        "action" => handle_action(&args[1..]),
        "supervisor" => handle_supervisor(&args[1..]),
        "shadow" => handle_shadow(&args[1..]),
        "parity" => handle_parity(&args[1..]),
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
    print_human(&format!(
        "Meridian Loom // INIT\n====================\nroot:        {}\nmode:        {}\norg_id:      {}\nstate_dir:   {}\nkernel_path: {}\nstatus:      initialized experimental scaffold\nnext_step:   loom doctor --root {} --format human\n",
        root.display(),
        config.mode,
        config.org_id,
        config.state_dir,
        if config.kernel_path.is_empty() { "(not set)" } else { &config.kernel_path },
        root.display()
    ));
    Ok(())
}

fn handle_doctor(args: &[String]) -> LoomResult<()> {
    let root = root_from(take_value(args, "--root").as_deref())?;
    let format = take_value(args, "--format").unwrap_or_else(|| "json".to_string());
    let checks = doctor(&root)?;
    match format.as_str() {
        "human" => print_human(&render_doctor_human(&checks)),
        _ => print!("{}", render_doctor_json(&checks)),
    }
    Ok(())
}

fn handle_health(args: &[String]) -> LoomResult<()> {
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

fn handle_status(args: &[String]) -> LoomResult<()> {
    let root = root_from(take_value(args, "--root").as_deref())?;
    print_human(&status_human(&root)?);
    Ok(())
}

fn handle_config(args: &[String]) -> LoomResult<()> {
    if args.first().map(String::as_str) != Some("show") {
        return Err("config only supports 'show' in this scaffold".to_string());
    }
    let root = root_from(take_value(args, "--root").as_deref())?;
    let config = read_config(&root)?;
    print_human(&render_config_human(&config, &root));
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
        print_human(&render_contract_human(&snapshot));
    }
    Ok(())
}

fn handle_capsule(args: &[String]) -> LoomResult<()> {
    if args.first().map(String::as_str) != Some("inspect") {
        return Err("capsule only supports 'inspect' in this scaffold".to_string());
    }
    let root = root_from(take_value(args, "--root").as_deref())?;
    let inspection = capsule_inspect(&root)?;
    print_human(&render_capsule_human(&inspection));
    Ok(())
}

fn handle_job(args: &[String]) -> LoomResult<()> {
    match args.first().map(String::as_str) {
        Some("list") => {
            let root = root_from(take_value(args, "--root").as_deref())?;
            let status_filter = take_value(args, "--status");
            let limit = take_value(args, "--limit")
                .and_then(|raw| raw.parse::<usize>().ok())
                .unwrap_or(20);
            let format = take_value(args, "--format").unwrap_or_else(|| "human".to_string());
            let jobs = list_jobs(&root, status_filter.as_deref(), limit)?;
            if format == "json" {
                print!("{}", render_job_list_json(&jobs));
            } else {
                print_human(&render_job_list_human(&root, &jobs, status_filter.as_deref()));
            }
            Ok(())
        }
        Some("inspect") => {
            let root = root_from(take_value(args, "--root").as_deref())?;
            let job_id = required_flag(args, "--job-id")?;
            let format = take_value(args, "--format").unwrap_or_else(|| "human".to_string());
            let snapshot = inspect_job(&root, &job_id)?;
            if format == "json" {
                print!("{}", render_job_inspect_json(&snapshot));
            } else {
                print_human(&render_job_inspect_human(&snapshot));
            }
            Ok(())
        }
        _ => Err("job supports 'list' and 'inspect'".to_string()),
    }
}

fn handle_agent(args: &[String]) -> LoomResult<()> {
    if args.first().map(String::as_str) != Some("resolve") {
        return Err("agent only supports 'resolve' in this scaffold".to_string());
    }
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

fn handle_envelope(args: &[String]) -> LoomResult<()> {
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

fn handle_shadow(args: &[String]) -> LoomResult<()> {
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
            let primary = PathBuf::from(required_flag(args, "--primary")?);
            let shadow = take_value(args, "--shadow")
                .map(PathBuf::from)
                .unwrap_or_else(|| root.join(".loom/shadow/events.jsonl"));
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

fn handle_action(args: &[String]) -> LoomResult<()> {
    match args.first().map(String::as_str) {
        Some("enqueue") => {
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
            let effective_kernel_path = kernel_path_for(&root, kernel_path.as_deref())?;
            let capture = enqueue_action(&root, &effective_kernel_path, &envelope)?;
            if format == "json" {
                print!("{}", render_enqueued_action_json(&capture));
            } else {
                print_human_block(&[
                    render_envelope_human(&envelope),
                    render_enqueued_action_human(&capture),
                ]);
            }
            Ok(())
        }
        Some("execute") => {
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
            let decision = capture_decision(&root, &identity, &envelope, &reference)?;
            let effective_kernel_path = kernel_path_for(&root, kernel_path.as_deref())?;
            let capture = capture_runtime_execution(
                &root,
                &effective_kernel_path,
                &envelope,
                &reference,
                &decision,
            )?;
            if format == "json" {
                print!("{}", render_runtime_execution_json(&capture));
            } else {
                print_human_block(&[
                    render_identity_human(&identity),
                    render_envelope_human(&envelope),
                    render_decision_human(&decision),
                    render_runtime_execution_human(&capture),
                ]);
            }
            std::process::exit(decision_exit_code(&decision, 0, 2));
        }
        _ => Err("action supports 'enqueue' and 'execute'".to_string()),
    }
}

fn handle_parity(args: &[String]) -> LoomResult<()> {
    match args.first().map(String::as_str) {
        Some("report") => {
            let root = root_from(take_value(args, "--root").as_deref())?;
            print_human(&render_parity_report(&root)?);
            Ok(())
        }
        _ => Err("parity supports 'report'".to_string()),
    }
}

fn handle_supervisor(args: &[String]) -> LoomResult<()> {
    match args.first().map(String::as_str) {
        Some("daemon") => handle_supervisor_daemon(&args[1..]),
        Some("run") => {
            let root = root_from(take_value(args, "--root").as_deref())?;
            let kernel_path = take_value(args, "--kernel-path");
            let max_jobs = take_value(args, "--max-jobs")
                .and_then(|raw| raw.parse::<usize>().ok())
                .unwrap_or(1);
            let format = take_value(args, "--format").unwrap_or_else(|| "human".to_string());
            let summary = run_supervisor(&root, kernel_path.as_deref(), max_jobs)?;
            if format == "json" {
                print!("{}", render_supervisor_run_json(&summary));
            } else {
                print_human(&render_supervisor_run_human(&summary));
            }
            Ok(())
        }
        Some("watch") => {
            let root = root_from(take_value(args, "--root").as_deref())?;
            let kernel_path = take_value(args, "--kernel-path");
            let max_jobs = take_value(args, "--max-jobs")
                .and_then(|raw| raw.parse::<usize>().ok())
                .unwrap_or(1);
            let iterations = take_value(args, "--iterations")
                .and_then(|raw| raw.parse::<usize>().ok())
                .unwrap_or(2);
            let poll_seconds = take_value(args, "--poll-seconds")
                .and_then(|raw| raw.parse::<u64>().ok())
                .unwrap_or(1);
            let format = take_value(args, "--format").unwrap_or_else(|| "human".to_string());
            let summary = watch_supervisor(
                &root,
                kernel_path.as_deref(),
                max_jobs,
                iterations,
                poll_seconds,
            )?;
            if format == "json" {
                print!("{}", render_supervisor_watch_json(&summary));
            } else {
                print_human(&render_supervisor_watch_human(&summary));
            }
            Ok(())
        }
        Some("status") => {
            let root = root_from(take_value(args, "--root").as_deref())?;
            let format = take_value(args, "--format").unwrap_or_else(|| "human".to_string());
            let snapshot = supervisor_status(&root)?;
            if format == "json" {
                print!("{}", render_supervisor_status_json(&snapshot));
            } else {
                print_human(&render_supervisor_status_human(&snapshot));
            }
            Ok(())
        }
        _ => Err("supervisor supports 'run', 'watch', 'status', and 'daemon'".to_string()),
    }
}

fn handle_supervisor_daemon(args: &[String]) -> LoomResult<()> {
    match args.first().map(String::as_str) {
        Some("start") => {
            let root = root_from(take_value(args, "--root").as_deref())?;
            let kernel_path = take_value(args, "--kernel-path");
            let max_jobs = take_value(args, "--max-jobs")
                .and_then(|raw| raw.parse::<usize>().ok())
                .unwrap_or(1);
            let poll_seconds = take_value(args, "--poll-seconds")
                .and_then(|raw| raw.parse::<u64>().ok())
                .unwrap_or(1);
            let iterations = take_value(args, "--iterations")
                .and_then(|raw| raw.parse::<usize>().ok())
                .unwrap_or(60);
            let format = take_value(args, "--format").unwrap_or_else(|| "human".to_string());
            let supervisor_dir = root.join(".loom/runtime/supervisor");
            std::fs::create_dir_all(&supervisor_dir).map_err(|e| e.to_string())?;
            let stdout_log_path = supervisor_dir.join("daemon.log");
            let stdout = std::fs::File::create(&stdout_log_path).map_err(|e| e.to_string())?;
            let stderr = stdout.try_clone().map_err(|e| e.to_string())?;
            let session_id = format!("daemon-{}", chrono_like_timestamp());
            let exe = env::current_exe().map_err(|e| e.to_string())?;
            let mut command = Command::new(exe);
            command
                .arg("supervisor")
                .arg("daemon")
                .arg("loop")
                .arg("--root")
                .arg(&root)
                .arg("--max-jobs")
                .arg(max_jobs.to_string())
                .arg("--poll-seconds")
                .arg(poll_seconds.to_string())
                .arg("--iterations")
                .arg(iterations.to_string())
                .arg("--session-id")
                .arg(&session_id)
                .stdout(Stdio::from(stdout))
                .stderr(Stdio::from(stderr));
            if let Some(kernel_path) = kernel_path.as_deref() {
                command.arg("--kernel-path").arg(kernel_path);
            }
            let child = command.spawn().map_err(|e| e.to_string())?;
            let note = format!(
                "daemon start requested; pid={} session_id={} log={}",
                child.id(),
                session_id,
                stdout_log_path.display()
            );
            let fallback_note = note.clone();
            let mut snapshot_result = supervisor_daemon_status(&root);
            for _ in 0..10 {
                if let Ok(snapshot) = &snapshot_result {
                    if snapshot.available {
                        break;
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
                snapshot_result = supervisor_daemon_status(&root);
            }
            let snapshot = snapshot_result.unwrap_or_else(|_| {
                loom_shadow::SupervisorDaemonSnapshot {
                    root: root.clone(),
                    supervisor_dir,
                    runtime_state_path: root.join(".loom/runtime/supervisor/runtime_state.json"),
                    stop_request_path: root.join(".loom/runtime/supervisor/stop.requested"),
                    stdout_log_path,
                    available: true,
                    session_id: session_id.clone(),
                    pid: child.id(),
                    running: true,
                    status: "starting".to_string(),
                    updated_at: String::new(),
                    booted_at: String::new(),
                    stopped_at: String::new(),
                    poll_seconds,
                    max_jobs,
                    max_iterations: iterations,
                    iterations_completed: 0,
                    processed: 0,
                    allowed: 0,
                    denied: 0,
                    failed: 0,
                    pending_jobs: 0,
                    processed_jobs: 0,
                    failed_jobs: 0,
                    heartbeat_entries: 0,
                    note: fallback_note,
                }
            });
            if format == "json" {
                print!("{}", render_supervisor_daemon_json(&snapshot));
            } else {
                let mut snapshot = snapshot;
                if snapshot.note.is_empty() {
                    snapshot.note = note;
                }
                print_human(&render_supervisor_daemon_human(&snapshot));
            }
            Ok(())
        }
        Some("loop") => {
            let root = root_from(take_value(args, "--root").as_deref())?;
            let kernel_path = take_value(args, "--kernel-path");
            let max_jobs = take_value(args, "--max-jobs")
                .and_then(|raw| raw.parse::<usize>().ok())
                .unwrap_or(1);
            let poll_seconds = take_value(args, "--poll-seconds")
                .and_then(|raw| raw.parse::<u64>().ok())
                .unwrap_or(1);
            let iterations = take_value(args, "--iterations")
                .and_then(|raw| raw.parse::<usize>().ok())
                .unwrap_or(60);
            let session_id =
                take_value(args, "--session-id").unwrap_or_else(|| format!("daemon-{}", chrono_like_timestamp()));
            let format = take_value(args, "--format").unwrap_or_else(|| "human".to_string());
            let snapshot = run_supervisor_daemon_loop(
                &root,
                kernel_path.as_deref(),
                max_jobs,
                poll_seconds,
                iterations,
                &session_id,
            )?;
            if format == "json" {
                print!("{}", render_supervisor_daemon_json(&snapshot));
            } else {
                print_human(&render_supervisor_daemon_human(&snapshot));
            }
            Ok(())
        }
        Some("status") => {
            let root = root_from(take_value(args, "--root").as_deref())?;
            let format = take_value(args, "--format").unwrap_or_else(|| "human".to_string());
            let snapshot = supervisor_daemon_status(&root)?;
            if format == "json" {
                print!("{}", render_supervisor_daemon_json(&snapshot));
            } else {
                print_human(&render_supervisor_daemon_human(&snapshot));
            }
            Ok(())
        }
        Some("stop") => {
            let root = root_from(take_value(args, "--root").as_deref())?;
            let format = take_value(args, "--format").unwrap_or_else(|| "human".to_string());
            let snapshot = request_supervisor_daemon_stop(&root)?;
            if format == "json" {
                print!("{}", render_supervisor_daemon_json(&snapshot));
            } else {
                print_human(&render_supervisor_daemon_human(&snapshot));
            }
            Ok(())
        }
        _ => Err("supervisor daemon supports 'start', 'loop', 'status', and 'stop'".to_string()),
    }
}

fn take_value(args: &[String], flag: &str) -> Option<String> {
    args.windows(2)
        .find(|pair| pair[0] == flag)
        .map(|pair| pair[1].clone())
}

fn required_flag(args: &[String], flag: &str) -> LoomResult<String> {
    take_value(args, flag).ok_or_else(|| format!("missing required flag {}", flag))
}

fn parse_f64_flag(args: &[String], flag: &str) -> Option<f64> {
    take_value(args, flag).and_then(|raw| raw.parse::<f64>().ok())
}

fn chrono_like_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string()
}

fn print_help() {
    print_human(
        "Meridian Loom // HELP\n\
======================\n\
phase:       public experimental scaffold\n\
boundary:    operator shape is real; governed runtime is not\n\
\n\
Bootstrap\n\
---------\n\
  loom init --mode <embedded|shadow|standalone> [--kernel-path PATH] [--root PATH] [--org-id ID]\n\
  loom doctor [--root PATH] [--format json|human]\n\
  loom health [--root PATH] [--format json|human]\n\
  loom status [--root PATH]\n\
  loom config show [--root PATH]\n\
\n\
Governance surfaces\n\
-------------------\n\
  loom contract show [--root PATH] [--kernel-path PATH] [--format human|json]\n\
  loom capsule inspect [--root PATH]\n\
  loom job list [--root PATH] [--status STATUS] [--limit N] [--format human|json]\n\
  loom job inspect --job-id HASH [--root PATH] [--format human|json]\n\
  loom agent resolve --agent-id ID [--org-id ORG] [--kernel-path PATH] [--root PATH] [--format human|json]\n\
  loom envelope build --agent-id ID --action-type TYPE --resource RESOURCE [--estimated-cost-usd USD] [--run-id ID] [--session-id ID] [--org-id ORG] [--kernel-path PATH] [--root PATH] [--format human|json]\n\
\n\
Runtime rehearsal\n\
-----------------\n\
  loom action enqueue --agent-id ID --action-type TYPE --resource RESOURCE [--estimated-cost-usd USD] [--run-id ID] [--session-id ID] [--org-id ORG] [--kernel-path PATH] [--root PATH] [--format human|json]\n\
  loom action execute --agent-id ID --action-type TYPE --resource RESOURCE [--estimated-cost-usd USD] [--run-id ID] [--session-id ID] [--org-id ORG] [--kernel-path PATH] [--root PATH] [--format human|json]\n\
  loom supervisor run [--root PATH] [--kernel-path PATH] [--max-jobs N] [--format human|json]\n\
  loom supervisor watch [--root PATH] [--kernel-path PATH] [--max-jobs N] [--iterations N] [--poll-seconds N] [--format human|json]\n\
  loom supervisor status [--root PATH] [--format human|json]\n\
  loom supervisor daemon start [--root PATH] [--kernel-path PATH] [--max-jobs N] [--poll-seconds N] [--iterations N] [--format human|json]\n\
  loom supervisor daemon status [--root PATH] [--format human|json]\n\
  loom supervisor daemon stop [--root PATH] [--format human|json]\n\
  loom shadow preflight --agent-id ID --action-type TYPE --resource RESOURCE [--estimated-cost-usd USD] [--run-id ID] [--session-id ID] [--org-id ORG] [--kernel-path PATH] [--root PATH] [--format human|json]\n\
  loom shadow decide --agent-id ID --action-type TYPE --resource RESOURCE [--estimated-cost-usd USD] [--run-id ID] [--session-id ID] [--org-id ORG] [--kernel-path PATH] [--root PATH] [--format human|json]\n\
  loom shadow enforce --agent-id ID --action-type TYPE --resource RESOURCE [--estimated-cost-usd USD] [--run-id ID] [--session-id ID] [--org-id ORG] [--kernel-path PATH] [--root PATH] [--format human|json]\n\
  loom shadow compare --primary FILE [--shadow FILE] [--root PATH] [--format human|json]\n\
  loom shadow report [--root PATH]\n\
  loom parity report [--root PATH]\n\
\n\
Next\n\
----\n\
  1. loom init --mode embedded --root /tmp/loom-rehearsal --kernel-path /tmp/meridian-kernel\n\
  2. loom action enqueue --agent-id agent_atlas --action-type research --resource web_search --root /tmp/loom-rehearsal\n\
  3. loom supervisor daemon start --root /tmp/loom-rehearsal --max-jobs 1 --poll-seconds 1 --iterations 10\n\
  4. loom job list --root /tmp/loom-rehearsal --format human\n\
  5. loom supervisor daemon status --root /tmp/loom-rehearsal --format human\n",
    );
}

fn print_human(output: &str) {
    if stdout_supports_color() {
        print!("{}", style_human_output(output));
    } else {
        print!("{}", output);
    }
}

fn print_human_block(parts: &[String]) {
    let merged = parts.join("\n\n");
    print_human(&merged);
}

fn stdout_supports_color() -> bool {
    if env::var_os("FORCE_COLOR").is_some() {
        return true;
    }
    if env::var_os("NO_COLOR").is_some() {
        return false;
    }
    io::stdout().is_terminal()
}

fn style_human_output(output: &str) -> String {
    let mut styled = output
        .lines()
        .map(style_human_line)
        .collect::<Vec<_>>()
        .join("\n");
    if output.ends_with('\n') {
        styled.push('\n');
    }
    styled
}

fn style_human_line(line: &str) -> String {
    const RESET: &str = "\x1b[0m";
    const CYAN: &str = "\x1b[38;5;81m";
    const BLUE: &str = "\x1b[38;5;111m";
    const GREEN: &str = "\x1b[38;5;114m";
    const YELLOW: &str = "\x1b[38;5;221m";
    const RED: &str = "\x1b[38;5;203m";
    const DIM: &str = "\x1b[2m";
    const BOLD: &str = "\x1b[1m";

    if line.starts_with("Meridian Loom //") {
        return format!("{BOLD}{CYAN}{line}{RESET}");
    }
    if !line.is_empty() && line.chars().all(|c| c == '=' || c == '-') {
        return format!("{DIM}{line}{RESET}");
    }
    if line.starts_with("[OK") {
        return format!("{GREEN}{line}{RESET}");
    }
    let lower = line.to_ascii_lowercase();
    if lower.contains("deny") || lower.contains("blocked") || lower.contains("failed") {
        return format!("{RED}{line}{RESET}");
    }
    if lower.contains("warn") || lower.contains("degraded") || lower.contains("divergence") {
        return format!("{YELLOW}{line}{RESET}");
    }
    if line.starts_with("phase:")
        || line.starts_with("boundary:")
        || line == "Decision"
        || line == "Checks"
        || line == "Current state"
        || line == "Next"
    {
        return format!("{BOLD}{BLUE}{line}{RESET}");
    }
    line.to_string()
}
