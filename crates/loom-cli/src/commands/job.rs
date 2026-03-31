use crate::*;

pub(crate) fn handle_job(args: &[String]) -> LoomResult<()> {
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
                print_human(&render_job_list_human(
                    &root,
                    &jobs,
                    status_filter.as_deref(),
                ));
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
        Some("approve") => {
            let root = root_from(take_value(args, "--root").as_deref())?;
            let job_id = required_flag(args, "--job-id")?;
            let result = approve_job(&root, &job_id)?;
            println!("{}", result);
            Ok(())
        }
        _ => Err("job supports 'list', 'inspect', and 'approve'".to_string()),
    }
}
