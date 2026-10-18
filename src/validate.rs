//! Semantic validation: turn a parsed [`LoadedConfig`] into a [`Project`].
//!
//! Everything the build needs is resolved here, once: tools, the default
//! recipe, each recipe's main document and output directory, and every step
//! with its templates already rendered. Later tasks therefore never look at
//! the raw TOML again.

use std::collections::BTreeMap;
use std::fmt;
use std::ops::Range;
use std::path::{Component, Path, PathBuf};

use toml::Spanned;

use crate::config::{LoadedConfig, RawRecipe, RawStep, RawTool};
use crate::error::ConfigError;
use crate::paths;
use crate::vars::{self, Vars};

/// A validated project configuration.
#[derive(Debug)]
pub struct Project {
    /// Absolute, lexically normalized project root.
    pub root: PathBuf,
    /// Absolute path of the configuration file.
    #[allow(dead_code)] // used in build diagnostics in later tasks
    pub config_path: PathBuf,
    /// Project name.
    pub name: String,
    /// The default recipe, if one is configured.
    pub default: Option<String>,
    /// Every recipe, keyed by name (sorted).
    pub recipes: BTreeMap<String, Recipe>,
}

/// One validated build recipe.
#[derive(Debug)]
pub struct Recipe {
    /// Recipe name, used in build diagnostics.
    #[allow(dead_code)] // read by build diagnostics in a later task
    pub name: String,
    /// Main document, relative to the project root.
    pub main: String,
    /// Main document file name without its extension.
    pub main_stem: String,
    /// Output directory, relative to the project root.
    pub output: String,
    /// Absolute output directory.
    pub output_path: PathBuf,
    /// Steps in execution order, all templates already rendered.
    pub steps: Vec<Step>,
}

/// One validated build step.
#[derive(Debug)]
pub enum Step {
    /// Run a LaTeX engine.
    Latex {
        /// Engine to run.
        engine: Engine,
        /// Extra arguments after the built-in ones.
        args: Vec<String>,
    },
    /// Run `biber`.
    Biber {
        /// Extra arguments after the built-in ones.
        args: Vec<String>,
    },
    /// Run `bibtex`.
    Bibtex {
        /// Extra arguments after the built-in ones.
        args: Vec<String>,
    },
    /// Run `makeindex` in the output directory.
    Makeindex {
        /// Extra arguments after the built-in ones.
        args: Vec<String>,
    },
    /// Run a shell command in the project root.
    Run {
        /// Complete command line, rendered.
        command: String,
    },
    /// Run a configured external tool.
    Tool {
        /// Executable name or path, rendered.
        command: String,
        /// Fixed arguments followed by the step's own arguments, rendered.
        args: Vec<String>,
    },
}

/// A LaTeX engine supported by the `latex` step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Engine {
    /// `pdflatex`.
    Pdflatex,
    /// `xelatex`.
    Xelatex,
    /// `lualatex`.
    Lualatex,
}

impl Engine {
    /// The executable name of the engine.
    pub fn command(self) -> &'static str {
        match self {
            Engine::Pdflatex => "pdflatex",
            Engine::Xelatex => "xelatex",
            Engine::Lualatex => "lualatex",
        }
    }
}

impl fmt::Display for Engine {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.command())
    }
}

/// The project-level values shared by every recipe.
struct ProjectMeta<'a> {
    /// Absolute, normalized project root.
    root: &'a Path,
    /// Resolved tool definitions.
    tools: &'a BTreeMap<String, ToolSpec>,
    /// Project-level main document.
    main: &'a Spanned<String>,
    /// Project-level output directory.
    output: Option<&'a Spanned<String>>,
    /// Project name.
    name: &'a str,
}

/// A validated `[tools.<name>]` definition.
struct ToolSpec {
    /// Executable name or path, unrendered.
    command: Spanned<String>,
    /// Fixed arguments, unrendered.
    args: Vec<Spanned<String>>,
}

/// Validate `loaded` and resolve it into a build plan.
///
/// Errors carry the file, line and column of the offending value.
pub fn build_plan(loaded: &LoadedConfig) -> Result<Project, ConfigError> {
    let root = paths::normalize(&loaded.root);
    let root = root.canonicalize().unwrap_or(root);

    let tools = parse_tools(loaded)?;

    if loaded.raw.recipes.is_empty() {
        return Err(loaded.error_at(
            loaded.raw.project.main.span(),
            "the configuration defines no recipes",
        ));
    }

    let project_name = loaded.raw.project.name.get_ref().clone();
    let project = ProjectMeta {
        root: &root,
        tools: &tools,
        main: &loaded.raw.project.main,
        output: loaded.raw.project.output.as_ref(),
        name: &project_name,
    };

    validate_tool_templates(loaded, &tools, &project)?;

    if let Some(default) = &loaded.raw.project.default
        && !loaded.raw.recipes.contains_key(default.get_ref())
    {
        return Err(loaded.error_at(
            default.span(),
            format!("the default recipe `{}` is not defined", default.get_ref()),
        ));
    }

    let mut recipes = BTreeMap::new();
    for (name, raw_recipe) in &loaded.raw.recipes {
        recipes.insert(
            name.clone(),
            build_recipe(loaded, &project, name, raw_recipe)?,
        );
    }

    Ok(Project {
        root,
        config_path: loaded.path.clone(),
        name: project_name,
        default: loaded
            .raw
            .project
            .default
            .as_ref()
            .map(|default| default.get_ref().clone()),
        recipes,
    })
}

fn build_recipe(
    loaded: &LoadedConfig,
    project: &ProjectMeta<'_>,
    name: &str,
    raw_recipe: &RawRecipe,
) -> Result<Recipe, ConfigError> {
    let main_span = raw_recipe.main.as_ref().unwrap_or(project.main);

    if raw_recipe.steps.is_empty() {
        return Err(loaded.error_at(main_span.span(), format!("recipe `{name}` has no `steps`")));
    }

    let main = main_span.get_ref().clone();
    validate_main(loaded, project.root, &main, main_span.span())?;

    let output_span = raw_recipe.output.as_ref().or(project.output);
    let output = output_span.map_or_else(|| "build".to_owned(), |span| span.get_ref().clone());
    let output_path = paths::validate_output(project.root, &output, &main).map_err(|reason| {
        loaded.error_at(
            output_span.map_or_else(|| main_span.span(), Spanned::span),
            reason,
        )
    })?;

    let main_stem = Path::new(&main)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or_default()
        .to_owned();

    let root_string = project.root.to_string_lossy().into_owned();
    let vars = Vars {
        root: &root_string,
        main: &main,
        main_stem: &main_stem,
        output: &output,
        name: project.name,
        recipe: name,
    };

    let mut steps = Vec::with_capacity(raw_recipe.steps.len());
    for raw_step in &raw_recipe.steps {
        steps.push(build_step(loaded, project.tools, &vars, raw_step)?);
    }

    Ok(Recipe {
        name: name.to_owned(),
        main,
        main_stem,
        output,
        output_path,
        steps,
    })
}

/// Render every tool template once, even when no step references the tool.
///
/// Templates are validated against a project-level [`Vars`] because variable
/// names do not depend on a recipe's values; this makes `check` report a bad
/// variable in an unused tool instead of silently accepting it.
fn validate_tool_templates(
    loaded: &LoadedConfig,
    tools: &BTreeMap<String, ToolSpec>,
    project: &ProjectMeta<'_>,
) -> Result<(), ConfigError> {
    let root = project.root.to_string_lossy().into_owned();
    let main = project.main.get_ref().clone();
    let main_stem = Path::new(&main)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or_default()
        .to_owned();
    let output = project
        .output
        .map_or_else(|| "build".to_owned(), |span| span.get_ref().clone());
    let vars = Vars {
        root: &root,
        main: &main,
        main_stem: &main_stem,
        output: &output,
        name: project.name,
        recipe: "",
    };
    for tool in tools.values() {
        render_at(
            loaded,
            tool.command.get_ref(),
            &vars,
            false,
            tool.command.span(),
        )?;
        for argument in &tool.args {
            render_at(loaded, argument.get_ref(), &vars, false, argument.span())?;
        }
    }
    Ok(())
}

/// Check that the main document is a direct `.tex` child of the project root
/// and that it exists.
fn validate_main(
    loaded: &LoadedConfig,
    root: &Path,
    main: &str,
    span: Range<usize>,
) -> Result<(), ConfigError> {
    let path = Path::new(main);
    let mut components = path.components();
    let direct_child =
        matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none();
    if !direct_child {
        return Err(loaded.error_at(
            span,
            format!("the main document `{main}` must be a direct child of the project root"),
        ));
    }
    if path.extension().and_then(|extension| extension.to_str()) != Some("tex") {
        return Err(loaded.error_at(
            span,
            format!("the main document `{main}` must have a `.tex` extension"),
        ));
    }
    if !root.join(main).is_file() {
        return Err(loaded.error_at(
            span,
            format!(
                "the main document `{main}` does not exist in {}",
                root.display()
            ),
        ));
    }
    Ok(())
}

fn build_step(
    loaded: &LoadedConfig,
    tools: &BTreeMap<String, ToolSpec>,
    vars: &Vars<'_>,
    raw_step: &Spanned<RawStep>,
) -> Result<Step, ConfigError> {
    let raw = raw_step.get_ref();

    let actions = [
        raw.latex.is_some(),
        raw.biber.is_some(),
        raw.bibtex.is_some(),
        raw.makeindex.is_some(),
        raw.run.is_some(),
        raw.tool.is_some(),
    ];
    let action_count = actions.iter().filter(|present| **present).count();
    if action_count != 1 {
        return Err(loaded.error_at(
            raw_step.span(),
            format!(
                "a step must have exactly one action key \
                 (latex, biber, bibtex, makeindex, run, tool); found {action_count}"
            ),
        ));
    }

    let extra_args = raw.args.as_deref().unwrap_or_default();

    if let Some(latex) = &raw.latex {
        let engine = match latex.get_ref().as_str() {
            "pdflatex" => Engine::Pdflatex,
            "xelatex" => Engine::Xelatex,
            "lualatex" => Engine::Lualatex,
            other => {
                return Err(loaded.error_at(
                    latex.span(),
                    format!(
                        "unknown latex engine `{other}`; \
                         expected `pdflatex`, `xelatex` or `lualatex`"
                    ),
                ));
            }
        };
        return Ok(Step::Latex {
            engine,
            args: render_args(loaded, vars, extra_args)?,
        });
    }

    if let Some(biber) = &raw.biber {
        ensure_true(loaded, biber, "biber")?;
        return Ok(Step::Biber {
            args: render_args(loaded, vars, extra_args)?,
        });
    }

    if let Some(bibtex) = &raw.bibtex {
        ensure_true(loaded, bibtex, "bibtex")?;
        return Ok(Step::Bibtex {
            args: render_args(loaded, vars, extra_args)?,
        });
    }

    if let Some(makeindex) = &raw.makeindex {
        ensure_true(loaded, makeindex, "makeindex")?;
        return Ok(Step::Makeindex {
            args: render_args(loaded, vars, extra_args)?,
        });
    }

    if let Some(run) = &raw.run {
        if raw.args.is_some() {
            return Err(loaded.error_at(
                run.span(),
                "`run` steps take the whole command; `args` is not allowed",
            ));
        }
        return Ok(Step::Run {
            command: render_at(loaded, run.get_ref(), vars, true, run.span())?,
        });
    }

    let tool_name = raw
        .tool
        .as_ref()
        .expect("exactly one action key is present");
    let tool = tools.get(tool_name.get_ref()).ok_or_else(|| {
        loaded.error_at(
            tool_name.span(),
            format!("unknown tool `{}`", tool_name.get_ref()),
        )
    })?;

    let command = render_at(
        loaded,
        tool.command.get_ref(),
        vars,
        false,
        tool.command.span(),
    )?;
    let mut args = Vec::with_capacity(tool.args.len() + extra_args.len());
    for argument in &tool.args {
        args.push(render_at(
            loaded,
            argument.get_ref(),
            vars,
            false,
            argument.span(),
        )?);
    }
    args.extend(render_args(loaded, vars, extra_args)?);

    Ok(Step::Tool { command, args })
}

/// Render a list of argument templates, reporting each argument's own span.
fn render_args(
    loaded: &LoadedConfig,
    vars: &Vars<'_>,
    args: &[Spanned<String>],
) -> Result<Vec<String>, ConfigError> {
    args.iter()
        .map(|argument| render_at(loaded, argument.get_ref(), vars, false, argument.span()))
        .collect()
}

/// Render one template, turning a render error into a positioned config error.
fn render_at(
    loaded: &LoadedConfig,
    template: &str,
    vars: &Vars<'_>,
    is_run: bool,
    span: Range<usize>,
) -> Result<String, ConfigError> {
    vars::render(template, vars, is_run).map_err(|reason| loaded.error_at(span, reason))
}

/// Require a `biber`/`bibtex`/`makeindex` value to be `true`.
fn ensure_true(loaded: &LoadedConfig, value: &Spanned<bool>, key: &str) -> Result<(), ConfigError> {
    if *value.get_ref() {
        Ok(())
    } else {
        Err(loaded.error_at(value.span(), format!("`{key}` must be `true`")))
    }
}

/// Resolve every `[tools.<name>]` definition.
fn parse_tools(loaded: &LoadedConfig) -> Result<BTreeMap<String, ToolSpec>, ConfigError> {
    let mut tools = BTreeMap::new();
    for (name, spanned) in &loaded.raw.tools {
        let tool = match spanned.get_ref() {
            RawTool::Command(command) => ToolSpec {
                command: Spanned::new(spanned.span(), command.clone()),
                args: Vec::new(),
            },
            RawTool::Table {
                command: Some(command),
                args,
            } => ToolSpec {
                command: command.clone(),
                args: args.clone().unwrap_or_default(),
            },
            RawTool::Table { command: None, .. } => {
                return Err(loaded.error_at(
                    spanned.span(),
                    format!("tool `{name}` is missing `command`"),
                ));
            }
        };
        tools.insert(name.clone(), tool);
    }
    Ok(tools)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load_config(dir: &Path, toml: &str) -> LoadedConfig {
        let path = dir.join("texmake.toml");
        std::fs::write(&path, toml).unwrap();
        crate::config::load(&path).unwrap()
    }

    #[test]
    fn builds_a_fully_rendered_recipe() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("main.tex"), "").unwrap();
        let loaded = load_config(
            tmp.path(),
            "[project]\nname = \"thesis\"\nmain = \"main.tex\"\noutput = \"build\"\n\n\
             [tools.graphviz]\ncommand = \"dot\"\nargs = [\"-Tpdf\", \"-o\", \"{output}/{main_stem}.pdf\"]\n\n\
             [recipes.release]\nsteps = [\n\
               { run = \"sh scripts/gen.sh {name}\" },\n\
               { tool = \"graphviz\", args = [\"{root}/graph.dot\"] },\n\
               { latex = \"xelatex\", args = [\"-shell-escape\", \"{output}/{main_stem}.pdf\"] },\n\
               { biber = true },\n\
             ]\n",
        );

        let project = build_plan(&loaded).unwrap();
        assert_eq!(project.root, tmp.path().canonicalize().unwrap());
        assert_eq!(project.name, "thesis");
        assert_eq!(project.default, None);

        let recipe = &project.recipes["release"];
        assert_eq!(recipe.name, "release");
        assert_eq!(recipe.main, "main.tex");
        assert_eq!(recipe.main_stem, "main");
        assert_eq!(recipe.output, "build");
        assert_eq!(recipe.output_path, project.root.join("build"));

        let root = project.root.display();
        assert!(matches!(
            &recipe.steps[0],
            Step::Run { command } if command == "sh scripts/gen.sh thesis"
        ));
        assert!(matches!(
            &recipe.steps[1],
            Step::Tool { command, args }
                if command == "dot"
                    && args == &vec![
                        "-Tpdf".to_owned(),
                        "-o".to_owned(),
                        "build/main.pdf".to_owned(),
                        format!("{root}/graph.dot"),
                    ]
        ));
        assert!(matches!(
            &recipe.steps[2],
            Step::Latex { engine: Engine::Xelatex, args }
                if args == &vec!["-shell-escape".to_owned(), "build/main.pdf".to_owned()]
        ));
        assert!(matches!(
            &recipe.steps[3],
            Step::Biber { args } if args.is_empty()
        ));
    }

    #[test]
    fn recipe_overrides_main_and_output() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("main.tex"), "").unwrap();
        std::fs::write(tmp.path().join("paper.tex"), "").unwrap();
        let loaded = load_config(
            tmp.path(),
            "[project]\nname = \"t\"\nmain = \"main.tex\"\n\n\
             [recipes.paper]\nmain = \"paper.tex\"\noutput = \"build/paper\"\n\
             steps = [{ latex = \"xelatex\" }]\n",
        );

        let project = build_plan(&loaded).unwrap();
        let recipe = &project.recipes["paper"];
        assert_eq!(recipe.main, "paper.tex");
        assert_eq!(recipe.main_stem, "paper");
        assert_eq!(recipe.output, "build/paper");
        assert_eq!(recipe.output_path, project.root.join("build/paper"));
    }

    #[test]
    fn normalizes_dotdot_left_in_the_configuration_root() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("sub")).unwrap();
        std::fs::write(tmp.path().join("main.tex"), "").unwrap();
        let path = tmp.path().join("sub/../texmake.toml");
        std::fs::write(
            &path,
            "[project]\nname = \"t\"\nmain = \"main.tex\"\n\n[recipes.a]\nsteps = [{ latex = \"xelatex\" }]\n",
        )
        .unwrap();

        let loaded = crate::config::load(&path).unwrap();
        let project = build_plan(&loaded).unwrap();
        assert_eq!(project.root, tmp.path().canonicalize().unwrap());
        assert_eq!(project.recipes["a"].output_path, project.root.join("build"));
    }
}
