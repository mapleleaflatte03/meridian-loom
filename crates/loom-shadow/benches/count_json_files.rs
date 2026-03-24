use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use tempfile::tempdir;
use walkdir::WalkDir;

fn count_json_files_recursive_old(path: &Path) -> Result<usize, String> {
    fn io_err(error: impl std::fmt::Display) -> String {
        error.to_string()
    }

    let mut count = 0usize;
    for entry in fs::read_dir(path).map_err(io_err)? {
        let entry = entry.map_err(io_err)?;
        let entry_path = entry.path();
        if entry_path.is_dir() {
            count += count_json_files_recursive_old(&entry_path)?;
        } else if entry_path
            .extension()
            .map(|ext| ext == "json")
            .unwrap_or(false)
        {
            count += 1;
        }
    }
    Ok(count)
}

fn count_json_files_recursive_new(path: &Path) -> Result<usize, String> {
    let mut count = 0usize;
    for entry in WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file()
            && entry.path().extension().map_or(false, |ext| ext == "json")
        {
            count += 1;
        }
    }
    Ok(count)
}

pub fn criterion_benchmark(c: &mut Criterion) {
    let dir = tempdir().unwrap();
    let root = dir.path().to_path_buf();

    // Create a reasonably deep and wide directory structure with some JSON files
    for i in 0..10 {
        let d = root.join(format!("dir_{}", i));
        fs::create_dir(&d).unwrap();
        for j in 0..20 {
            File::create(d.join(format!("file_{}.json", j))).unwrap();
        }
        let sub = d.join("sub");
        fs::create_dir(&sub).unwrap();
        for j in 0..20 {
            File::create(sub.join(format!("subfile_{}.json", j))).unwrap();
            File::create(sub.join(format!("subfile_{}.txt", j))).unwrap();
        }
    }

    let mut group = c.benchmark_group("count_json_files");
    group.bench_function("old", |b| {
        b.iter(|| count_json_files_recursive_old(black_box(&root)))
    });
    group.bench_function("new", |b| {
        b.iter(|| count_json_files_recursive_new(black_box(&root)))
    });
    group.finish();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
