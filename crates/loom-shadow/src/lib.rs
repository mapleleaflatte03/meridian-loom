use std::fs;
use std::path::Path;

pub type ShadowResult<T> = Result<T, String>;

pub fn render_shadow_report(root: &Path) -> ShadowResult<String> {
    let report_path = root.join(".loom/shadow/latest.json");
    let contents = fs::read_to_string(&report_path)
        .map_err(|error| format!("could not read {}: {}", report_path.display(), error))?;
    Ok(format!(
        "Shadow report\n=============\nsource: {}\n\n{}\n",
        report_path.display(),
        contents
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn renders_existing_report() {
        let root = std::env::temp_dir().join(format!(
            "loom-shadow-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
        ));
        let report_dir = root.join(".loom/shadow");
        fs::create_dir_all(&report_dir).expect("report dir");
        fs::write(report_dir.join("latest.json"), "{\"status\":\"not_started\"}\n").expect("write report");
        let rendered = render_shadow_report(&root).expect("render");
        assert!(rendered.contains("not_started"));
    }
}

