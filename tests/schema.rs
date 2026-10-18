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

//! Guard `schema/texmake.schema.json` against drifting away from the
//! configuration types in `src/config.rs` and the rules in `src/validate.rs`.

use serde_json::Value;

const SCHEMA_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/schema/texmake.schema.json");

fn schema() -> Value {
    let text = std::fs::read_to_string(SCHEMA_PATH)
        .expect("the configuration schema file must exist and be readable");
    serde_json::from_str(&text).expect("the configuration schema must be valid JSON")
}

/// The `steps.items` schema inside a recipe.
fn step_schema(schema: &Value) -> &Value {
    &schema["properties"]["recipes"]["additionalProperties"]["properties"]["steps"]["items"]
}

/// The six action shapes a step may take.
fn step_actions(schema: &Value) -> &Vec<Value> {
    step_schema(schema)["oneOf"]
        .as_array()
        .expect("a step must be expressed as a oneOf array")
}

fn action_branch<'a>(branches: &'a [Value], action: &str) -> &'a Value {
    branches
        .iter()
        .find(|branch| branch["properties"][action].is_object())
        .unwrap_or_else(|| panic!("no step branch declares the `{action}` action"))
}

#[test]
fn schema_declares_draft_2020_12_and_its_identity() {
    let schema = schema();
    assert_eq!(
        schema["$schema"], "https://json-schema.org/draft/2020-12/schema",
        "the schema must target JSON Schema draft 2020-12"
    );
    assert_eq!(
        schema["$id"], "https://github.com/JouderMin/texmake-rs/schema/texmake.schema.json",
        "the schema must carry its canonical id"
    );
    assert_eq!(schema["title"], "texmake 配置");
    assert_eq!(schema["type"], "object");
}

#[test]
fn root_requires_project_and_rejects_unknown_keys() {
    let schema = schema();
    let required = schema["required"]
        .as_array()
        .expect("the root object must list required keys");
    assert!(
        required.iter().any(|key| key == "project"),
        "`project` must be required: {required:?}"
    );
    assert_eq!(
        schema["additionalProperties"], false,
        "unknown root keys must be rejected"
    );

    let project = &schema["properties"]["project"];
    assert_eq!(project["type"], "object");
    assert_eq!(project["additionalProperties"], false);
    for key in ["name", "main"] {
        assert!(
            project["required"]
                .as_array()
                .is_some_and(|required| required.iter().any(|entry| entry == key)),
            "`project.{key}` must be required"
        );
    }
}

#[test]
fn recipes_require_steps_and_keep_at_least_one_recipe() {
    let recipes = &schema()["properties"]["recipes"];
    assert_eq!(recipes["type"], "object");
    assert_eq!(recipes["minProperties"], 1);
    let recipe = &recipes["additionalProperties"];
    assert_eq!(
        recipe["additionalProperties"], false,
        "a recipe must reject unknown keys"
    );
    assert!(
        recipe["required"]
            .as_array()
            .is_some_and(|required| required.iter().any(|key| key == "steps")),
        "every recipe must require `steps`"
    );
    let steps = &recipe["properties"]["steps"];
    assert_eq!(steps["type"], "array");
    assert_eq!(steps["minItems"], 1);
    assert_eq!(
        step_schema(&schema())["oneOf"].as_array().map(Vec::len),
        Some(6)
    );
}

#[test]
fn step_actions_form_a_six_way_one_of() {
    let schema = schema();
    let branches = step_actions(&schema);
    assert_eq!(branches.len(), 6, "a step must offer exactly six actions");

    let actions = ["latex", "biber", "bibtex", "makeindex", "run", "tool"];
    for action in actions {
        let branch = action_branch(branches, action);
        assert_eq!(branch["type"], "object", "`{action}` must be an object");
        assert_eq!(
            branch["additionalProperties"], false,
            "the `{action}` branch must reject unknown keys"
        );
        let required = branch["required"]
            .as_array()
            .unwrap_or_else(|| panic!("the `{action}` branch must list a required key"));
        assert_eq!(required.len(), 1, "`{action}` must require exactly one key");
        assert_eq!(required[0], action);
    }
}

#[test]
fn boolean_step_actions_are_required_true() {
    let schema = schema();
    let branches = step_actions(&schema);
    for action in ["biber", "bibtex", "makeindex"] {
        let branch = action_branch(branches, action);
        assert_eq!(
            branch["properties"][action]["const"], true,
            "`{action}` must be the constant `true`"
        );
    }
    let latex = action_branch(branches, "latex");
    let engines = latex["properties"]["latex"]["enum"]
        .as_array()
        .expect("the latex action must enumerate its engines");
    let engines: Vec<&str> = engines
        .iter()
        .enumerate()
        .map(|(index, value)| {
            value
                .as_str()
                .unwrap_or_else(|| panic!("engine enum item {index} is not a string: {value}"))
        })
        .collect();
    assert_eq!(engines, ["pdflatex", "xelatex", "lualatex"]);
}

#[test]
fn run_step_does_not_accept_args() {
    let schema = schema();
    let branches = step_actions(&schema);
    let run = action_branch(branches, "run");
    assert!(
        run["properties"].get("args").is_none(),
        "a `run` step takes the whole command and must not accept `args`"
    );
    assert_eq!(
        run["additionalProperties"], false,
        "`args` on a `run` step must be rejected as an unknown key"
    );
}

#[test]
fn tools_accept_a_command_string_or_a_table() {
    let schema = schema();
    let tools = &schema["properties"]["tools"];
    assert_eq!(tools["type"], "object");
    let shapes = tools["additionalProperties"]["oneOf"]
        .as_array()
        .expect("a tool must be either a string or a table");
    assert_eq!(shapes.len(), 2);
    assert!(
        shapes.iter().any(|shape| shape["type"] == "string"),
        "the string shorthand must stay supported"
    );
    let table = shapes
        .iter()
        .find(|shape| shape["type"] == "object")
        .expect("the table form must stay supported");
    assert_eq!(table["additionalProperties"], false);
    assert!(
        table["required"]
            .as_array()
            .is_some_and(|required| required.iter().any(|key| key == "command")),
        "a tool table must require `command`"
    );
    assert_eq!(table["properties"]["args"]["items"]["type"], "string");
}
