use serde_json::Value;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

fn unique_temp_dir(label: &str) -> PathBuf {
    let n = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!(
        "loom_connect_{}_{}_{}",
        label,
        std::process::id(),
        n
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

struct Harness {
    home: PathBuf,
    root: PathBuf,
    kernel: PathBuf,
}

impl Harness {
    fn new(label: &str) -> Self {
        let home = unique_temp_dir(label);
        let root = home.join(".local/share/meridian-loom/runtime/default");
        let kernel = home.join("kernel");
        scaffold_kernel_fixture(&kernel);
        let harness = Self { home, root, kernel };
        harness.run_ok(&[
            "init",
            "--mode",
            "embedded",
            "--root",
            harness.root_str(),
            "--kernel-path",
            harness.kernel_str(),
            "--org-id",
            "org_demo",
        ]);
        harness
    }

    fn root_str(&self) -> &str {
        self.root.to_str().expect("root str")
    }

    fn kernel_str(&self) -> &str {
        self.kernel.to_str().expect("kernel str")
    }

    fn config_home(&self) -> PathBuf {
        self.home.join(".config")
    }

    fn binary(&self) -> &'static str {
        env!("CARGO_BIN_EXE_loom")
    }

    fn base_command(&self) -> Command {
        let mut command = Command::new(self.binary());
        command.env("HOME", &self.home);
        command.env("XDG_CONFIG_HOME", self.config_home());
        command
    }

    fn run_output(&self, args: &[&str]) -> Output {
        self.base_command()
            .args(args)
            .output()
            .expect("run loom command")
    }

    fn run_ok(&self, args: &[&str]) -> String {
        let output = self.run_output(args);
        assert_success(args, &output);
        String::from_utf8_lossy(&output.stdout).to_string()
    }

    fn run_fail(&self, args: &[&str]) -> String {
        let output = self.run_output(args);
        assert!(
            !output.status.success(),
            "command {:?} unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
            args,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
        format!(
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        )
    }

    fn json_ok(&self, args: &[&str]) -> Value {
        let output = self.run_output(args);
        assert_success(args, &output);
        serde_json::from_slice(&output.stdout).expect("parse json")
    }
}

fn assert_success(args: &[&str], output: &Output) {
    assert!(
        output.status.success(),
        "command {:?} failed\nstatus: {:?}\nstdout:\n{}\nstderr:\n{}",
        args,
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

fn scaffold_kernel_fixture(root: &Path) {
    let kernel_dir = root.join("kernel");
    fs::create_dir_all(&kernel_dir).expect("kernel dir");
    fs::write(
        kernel_dir.join("runtimes.json"),
        "{\n  \"runtimes\": {\n    \"local_kernel\": {\"id\": \"local_kernel\", \"label\": \"Local Kernel Runtime\"},\n    \"loom_native\": {\"status\": \"official\", \"notes\": \"connect fixture\", \"contract_compliance\": {\"agent_identity\": true, \"action_envelope\": true, \"cost_attribution\": true, \"approval_hook\": true, \"audit_emission\": true, \"sanction_controls\": true, \"budget_gate\": true}}\n  }\n}\n",
    )
    .expect("write runtimes");
    fs::write(
        kernel_dir.join("agent_registry.py"),
        "import json, sys\nagent_id = sys.argv[sys.argv.index('--agent_id') + 1]\norg_id = sys.argv[sys.argv.index('--org_id') + 1] if '--org_id' in sys.argv else 'org_demo'\nprint(json.dumps({'id': agent_id, 'name': 'Atlas', 'org_id': org_id, 'role': 'analyst', 'economy_key': 'atlas', 'approval_required': False, 'budget': {'max_per_run_usd': 1.0}, 'runtime_binding': {'runtime_id': 'loom_native', 'runtime_label': 'Loom Native Runtime', 'bound_org_id': org_id, 'boundary_name': 'workspace', 'identity_model': 'session', 'runtime_registered': True, 'registration_status': 'registered'}}, indent=2))\n",
    )
    .expect("write registry");
    fs::write(
        kernel_dir.join("court.py"),
        "def get_restrictions(agent_id, org_id=None):\n    return []\n",
    )
    .expect("write court");
    fs::write(
        kernel_dir.join("authority.py"),
        "def check_authority(agent_id, action, org_id=None):\n    return True, 'ok'\n",
    )
    .expect("write authority");
}

#[test]
fn connect_scaffold_creates_manifest_with_poge_standard_fields() {
    let harness = Harness::new("scaffold_manifest");
    let output = harness.json_ok(&[
        "connect",
        "scaffold",
        "--name",
        "grpc_action_adapter",
        "--transport",
        "grpc",
        "--action-schema",
        "meridian.a2a.action.v1",
        "--root",
        harness.root_str(),
        "--format",
        "json",
    ]);
    assert_eq!(
        output.get("status").and_then(Value::as_str),
        Some("connect_scaffolded")
    );
    let manifest_path = output
        .get("manifest_path")
        .and_then(Value::as_str)
        .expect("manifest_path");
    let manifest: Value = serde_json::from_str(
        &fs::read_to_string(manifest_path).expect("read connect manifest"),
    )
    .expect("parse connect manifest");
    assert_eq!(
        manifest.get("transport").and_then(Value::as_str),
        Some("grpc")
    );
    assert_eq!(
        manifest.get("action_schema").and_then(Value::as_str),
        Some("meridian.a2a.action.v1")
    );
    assert_eq!(
        manifest
            .pointer("/poge_standard/warrant_required")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        manifest
            .pointer("/poge_standard/treasury_gate")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        manifest
            .pointer("/poge_standard/zk_settlement_compatible")
            .and_then(Value::as_bool),
        Some(true)
    );
}

#[test]
fn connect_list_surfaces_all_supported_transport_profiles() {
    let harness = Harness::new("transport_list");
    for (name, transport) in [
        ("grpc_adapter", "grpc"),
        ("a2a_adapter", "a2a"),
        ("mcp_adapter", "mcp"),
        ("http_adapter", "http"),
        ("ros2_adapter", "ros2"),
    ] {
        harness.run_ok(&[
            "connect",
            "scaffold",
            "--name",
            name,
            "--transport",
            transport,
            "--action-schema",
            "meridian.a2a.action.v1",
            "--root",
            harness.root_str(),
            "--format",
            "json",
        ]);
    }
    let listed = harness.json_ok(&["connect", "list", "--root", harness.root_str(), "--format", "json"]);
    let adapters = listed
        .get("adapters")
        .and_then(Value::as_array)
        .expect("adapters array");
    let transports = adapters
        .iter()
        .filter_map(|adapter| adapter.get("transport").and_then(Value::as_str))
        .map(|value| value.to_string())
        .collect::<BTreeSet<_>>();
    let expected = ["grpc", "a2a", "mcp", "http", "ros2"]
        .iter()
        .map(|value| value.to_string())
        .collect::<BTreeSet<_>>();
    assert_eq!(transports, expected);
}

#[test]
fn connect_scaffold_upserts_existing_adapter_without_duplicates() {
    let harness = Harness::new("upsert");
    harness.run_ok(&[
        "connect",
        "scaffold",
        "--name",
        "shared_adapter",
        "--transport",
        "http",
        "--action-schema",
        "meridian.a2a.action.v1",
        "--root",
        harness.root_str(),
        "--format",
        "json",
    ]);
    harness.run_ok(&[
        "connect",
        "scaffold",
        "--name",
        "shared_adapter",
        "--transport",
        "mcp",
        "--action-schema",
        "meridian.a2a.action.v1",
        "--root",
        harness.root_str(),
        "--format",
        "json",
    ]);
    let listed = harness.json_ok(&["connect", "list", "--root", harness.root_str(), "--format", "json"]);
    let adapters = listed
        .get("adapters")
        .and_then(Value::as_array)
        .expect("adapters array");
    let shared = adapters
        .iter()
        .filter(|adapter| adapter.get("adapter_id").and_then(Value::as_str) == Some("shared-adapter"))
        .collect::<Vec<_>>();
    assert_eq!(shared.len(), 1, "expected single upserted adapter");
    assert_eq!(
        shared[0].get("transport").and_then(Value::as_str),
        Some("mcp")
    );
}

#[test]
fn connect_scaffold_rejects_unknown_transport() {
    let harness = Harness::new("reject_transport");
    let stderr = harness.run_fail(&[
        "connect",
        "scaffold",
        "--name",
        "bad_adapter",
        "--transport",
        "ftp",
        "--action-schema",
        "meridian.a2a.action.v1",
        "--root",
        harness.root_str(),
        "--format",
        "json",
    ]);
    assert!(
        stderr.contains("unsupported transport"),
        "expected unsupported transport error, got:\n{}",
        stderr
    );
}
