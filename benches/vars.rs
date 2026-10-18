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

//! Variable rendering benchmarks.

use std::time::Duration;

use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use texmake::vars::{Vars, render};

fn sample_vars() -> Vars<'static> {
    Vars {
        root: "/projects/thesis",
        main: "main.tex",
        main_stem: "main",
        output: "build",
        name: "thesis",
        recipe: "release",
    }
}

fn bench_vars(c: &mut Criterion) {
    let vars = sample_vars();
    let mut group = c.benchmark_group("vars");
    group
        .sample_size(50)
        .warm_up_time(Duration::from_millis(300))
        .measurement_time(Duration::from_secs(2));

    group.bench_function("single_variable", |b| {
        b.iter(|| render(black_box("{output}/main.pdf"), &vars, false))
    });
    group.bench_function("latex_command", |b| {
        let template = "xelatex -interaction=nonstopmode -halt-on-error -file-line-error \
                       -output-directory={output} {main}";
        b.iter(|| render(black_box(template), &vars, false))
    });
    group.bench_function("passthrough_braces", |b| {
        b.iter(|| render(black_box("awk '{print $1}' {root}/list.txt"), &vars, false))
    });
    group.bench_function("long_template", |b| {
        let template = "xelatex {root}/{main} {output} {name} {recipe} ".repeat(24);
        b.iter(|| render(black_box(&template), &vars, false))
    });
    group.bench_function("unknown_variable", |b| {
        b.iter(|| render(black_box("{bogus}"), &vars, false))
    });

    group.finish();
}

criterion_group!(benches, bench_vars);
criterion_main!(benches);
