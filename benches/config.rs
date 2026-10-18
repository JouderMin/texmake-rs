// texmake - a make-like build tool for LaTeX document projects
// Copyright (C) 2026 texmake contributors
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

#![allow(clippy::unwrap_used)]

//! Configuration loading and validation benchmarks.

use std::fs;
use std::time::Duration;

use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use tempfile::TempDir;
use texmake::{config, recipe, validate};

const CONFIG: &str = r#"
[project]
name = "thesis"
main = "main.tex"
output = "build"
default = "release"

[tools.graphviz]
command = "dot"
args = ["-Tpdf", "-o", "{output}/graph.pdf"]

[tools]
plantuml = "plantuml"

[recipes.release]
steps = [
  { latex = "xelatex" },
  { biber = true },
  { latex = "xelatex" },
  { tool = "graphviz", args = ["{root}/graph.dot"] },
  { latex = "xelatex" },
  { latex = "xelatex" },
  { latex = "xelatex" },
  { tool = "plantuml", args = ["{root}/diagram.puml"] },
]

[recipes.draft]
steps = [{ latex = "xelatex", args = ["-draftmode"] }]

[recipes.print]
output = "build/print"
steps = [
  { latex = "lualatex" },
  { biber = true },
  { latex = "lualatex" },
  { makeindex = true },
  { latex = "lualatex" },
  { run = "cp {output}/{main_stem}.pdf {root}/print.pdf" },
]

[recipes.preview]
steps = [
  { latex = "pdflatex" },
  { bibtex = true },
  { latex = "pdflatex" },
  { latex = "pdflatex" },
]

[recipes.notes]
steps = [{ run = "echo building {name} ({recipe})" }]
"#;

fn fixture() -> TempDir {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("texmake.toml"), CONFIG).unwrap();
    fs::write(dir.path().join("main.tex"), "\\documentclass{article}\n").unwrap();
    dir
}

fn bench_config(c: &mut Criterion) {
    let dir = fixture();
    let config_path = dir.path().join("texmake.toml");
    let loaded = config::load(&config_path).unwrap();

    let mut group = c.benchmark_group("config");
    group
        .sample_size(40)
        .warm_up_time(Duration::from_millis(300))
        .measurement_time(Duration::from_secs(2));

    group.bench_function("load_and_parse", |b| {
        b.iter(|| config::load(black_box(&config_path)).unwrap())
    });
    group.bench_function("validate", |b| {
        b.iter(|| validate::build_plan(black_box(&loaded)).unwrap())
    });
    group.bench_function("load_and_validate", |b| {
        b.iter(|| {
            let loaded = config::load(black_box(&config_path)).unwrap();
            validate::build_plan(&loaded).unwrap()
        })
    });

    let project = validate::build_plan(&loaded).unwrap();
    let release = project.recipes.get("release").unwrap();
    group.bench_function("resolve_steps", |b| {
        b.iter(|| recipe::resolve_steps(black_box(&project), black_box(release)))
    });

    group.finish();
}

criterion_group!(benches, bench_config);
criterion_main!(benches);
