#![allow(clippy::unwrap_used)]

//! Up-to-date scanning benchmarks on a synthetic project.

use std::fs;
use std::time::Duration;

use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use tempfile::TempDir;
use texmake::config;
use texmake::up_to_date::{is_up_to_date, newest_source_mtime};
use texmake::validate::{Project, build_plan};

const CONFIG: &str = r#"
[project]
name = "thesis"
main = "main.tex"
default = "release"

[recipes.release]
steps = [{ latex = "xelatex" }]
"#;

const FILES: usize = 2_000;

/// Build a project with `FILES` source files spread over 25 directories.
fn fixture() -> (TempDir, Project) {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("texmake.toml"), CONFIG).unwrap();
    fs::write(dir.path().join("main.tex"), "\\documentclass{article}\n").unwrap();

    let build = dir.path().join("build");
    fs::create_dir(&build).unwrap();
    fs::write(build.join("main.pdf"), b"%PDF-1.7\n").unwrap();

    for i in 0..FILES {
        let sub = dir.path().join(format!("chapters/part{}", i % 25));
        fs::create_dir_all(&sub).unwrap();
        fs::write(sub.join(format!("section{i}.tex")), "\\section{}\n").unwrap();
    }

    let loaded = config::load(&dir.path().join("texmake.toml")).unwrap();
    let project = build_plan(&loaded).unwrap();
    (dir, project)
}

fn bench_scan(c: &mut Criterion) {
    let (_dir, project) = fixture();
    let recipe = project.recipes.get("release").unwrap();

    let mut group = c.benchmark_group("scan");
    group
        .sample_size(40)
        .warm_up_time(Duration::from_millis(300))
        .measurement_time(Duration::from_secs(2));

    group.bench_function(format!("is_up_to_date_{FILES}_files"), |b| {
        b.iter(|| is_up_to_date(black_box(&project), black_box(recipe)))
    });
    group.bench_function(format!("newest_source_mtime_{FILES}_files"), |b| {
        b.iter(|| newest_source_mtime(black_box(&project)))
    });

    group.finish();
}

criterion_group!(benches, bench_scan);
criterion_main!(benches);
