use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::fs;
use std::io::Write;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;

fn create_test_file() -> tempfile::NamedTempFile {
    let mut file = NamedTempFile::new().unwrap();
    let line = "{\"hook_name\":\"agent_identity\",\"input_hash\":\"abc\",\"decision\":\"resolved\",\"agent_id\":\"agent_atlas\",\"org_id\":\"org_demo\"}\n";
    let content = line.repeat(100_000); // Create a large file
    file.write_all(content.as_bytes()).unwrap();
    file
}

fn bench_load_events(c: &mut Criterion) {
    let file = create_test_file();
    let path = file.path();
    c.bench_function("load_events", |b| {
        b.iter(|| {
            let contents = fs::read_to_string(&path).unwrap();
            let mut events = Vec::new();
            for line in contents.lines() {
                let raw = line.trim();
                if raw.is_empty() {
                    continue;
                }
                events.push(raw.to_string());
            }
            black_box(events)
        })
    });
}

fn bench_load_events_buffered(c: &mut Criterion) {
    let file = create_test_file();
    let path = file.path();
    c.bench_function("load_events_buffered", |b| {
        b.iter(|| {
            let f = fs::File::open(&path).unwrap();
            let reader = BufReader::new(f);
            let mut events = Vec::new();
            for line in reader.lines() {
                let line = line.unwrap();
                let raw = line.trim();
                if raw.is_empty() {
                    continue;
                }
                events.push(raw.to_string());
            }
            black_box(events)
        })
    });
}

criterion_group!(benches, bench_load_events, bench_load_events_buffered);
criterion_main!(benches);
