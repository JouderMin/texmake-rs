# texmake

texmake 是 LaTeX 项目的构建工具，用法类似 make。在 `texmake.toml` 中声明构建顺序；texmake 按顺序执行步骤，遇到第一个失败的步骤时停止。

命令行帮助和错误信息为英文。

## 安装

```sh
cargo install --path .
# 或在 target/release/texmake 生成可执行文件
cargo build --release
```

## 快速开始

```sh
texmake init thesis   # 创建 texmake.toml、main.tex 和 .gitignore
cd thesis
texmake               # 按默认配方构建，输出 build/main.pdf
texmake check         # 检查配置
texmake clean         # 删除所有配方的输出目录
```

`texmake init [dir]` 创建项目，支持以下选项：

- `--name <name>`：项目名。未指定时取目标目录名。目标目录没有目录名（例如 `/`）时报错，并要求用 `--name` 指定。
- `--engine <pdflatex|xelatex|lualatex>`：生成配方使用的引擎，默认 `xelatex`。
- `--language <zh|en>`：生成的 `main.tex` 使用的语言，默认 `zh`。

init 生成 `texmake.toml`、`main.tex` 和 `.gitignore`，其中 `.gitignore` 的内容为 `/build/`。已有 `texmake.toml` 时，init 报错，不覆盖文件。其他已有文件保留，并显示为跳过；同名路径已存在但不是普通文件（例如目录）时报错，且不写任何文件。`--engine pdflatex --language zh` 报错：pdflatex 不能排版中文。`-n` 只显示将创建或跳过的文件；`-q` 不显示进度信息。

## 配置

texmake 从当前目录向上查找最近的 `texmake.toml`。`--file <path>` 指定其他配置文件；相对路径按当前目录解析，文件必须存在。配置文件所在目录是项目根，所有相对路径按项目根解析。

```toml
[project]
name = "thesis"
main = "main.tex"
output = "build"
default = "release"

[tools.graphviz]
command = "dot"
args = ["-Tpdf", "-o", "{output}/graph.pdf"]

[tools]
pandoc = "pandoc"

[recipes.release]
steps = [
  { latex = "xelatex" },
  { biber = true },
  { latex = "xelatex" },
  { tool = "graphviz", args = ["{root}/graph.dot"] },
]

[recipes.draft]
output = "build/draft"
steps = [{ latex = "xelatex", args = ["-draftmode"] }]
```

`[project]` 的键：`name` 和 `main` 必需；`output` 可选，默认 `build`；`default` 可选，指定默认配方。每个配方的 `steps` 必须非空；配方可用 `main` 和 `output` 覆盖项目级设置。

未知的键名、缺失的必需键、类型错误和重复键名都是配置错误。配置错误报告文件路径、行、列和原因。

## 命令

| 命令 | 作用 |
|---|---|
| `texmake` | 构建默认配方。 |
| `texmake build [name]` | 构建默认配方或指定配方。 |
| `texmake init [dir]` | 创建新项目。 |
| `texmake clean` | 删除所有配方的输出目录。 |
| `texmake check` | 检查配置，打印项目名和配方列表。 |

未指定配方名时，选择规则如下：

- 配置了 `default`：使用 `default` 指定的配方。
- 没有 `default` 且只有一个配方：使用这个配方。
- 其他情况：报错，并要求指定配方名。

指定的配方名不存在时报错，并列出全部配方名。

`texmake check` 输出项目名和按名称排序的配方列表，例如：

```text
project: thesis
recipes:
  draft
  release (default)
```

`texmake clean` 删除各配方的输出目录。嵌套的输出目录通过父目录删除；目录不存在时跳过。`-n` 只打印 `would remove <路径>`；`-q` 不打印 `removed <路径>`。输出目录是符号链接时，clean 报错，退出码为 2，不删除链接或链接目标。

警告：clean 删除输出目录及其内容，删除后不能恢复。

## 选项

| 选项 | 作用 |
|---|---|
| `-f, --file <path>` | 使用指定的配置文件。 |
| `-B, --force` | 忽略最新判断，强制构建。 |
| `-n, --dry-run` | 只打印命令，不执行。同时跳过最新判断和工具检查。 |
| `-v, --verbose` | 打印完整命令，并实时输出工具的输出。 |
| `-q, --quiet` | 只输出错误。 |

以上选项都是全局选项，可以放在子命令之前或之后。

## 内置步骤

| 键 | 示例 | 行为 |
|---|---|---|
| `latex` | `{ latex = "xelatex" }` | 在项目根运行 `pdflatex`、`xelatex` 或 `lualatex`。参数顺序：`-interaction=nonstopmode`、`-halt-on-error`、`-file-line-error`、`-output-directory=<输出目录>`、步骤的 `args`、主文档。 |
| `biber` | `{ biber = true }` | 在项目根运行 `biber`。参数：步骤的 `args`、`<输出目录>/<主文档名去掉扩展名>`。 |
| `bibtex` | `{ bibtex = true }` | 与 `biber` 相同，运行 `bibtex`。 |
| `makeindex` | `{ makeindex = true }` | 在输出目录运行 `makeindex`。参数：步骤的 `args`、主文档名去掉扩展名。 |
| `run` | `{ run = "sh scripts/gen.sh" }` | 在项目根通过系统 shell 执行整条命令。`args` 不允许。支持管道、重定向等 shell 语法。 |
| `tool` | `{ tool = "graphviz", args = ["{root}/graph.dot"] }` | 直接运行自定义工具，不经过 shell。参数为先工具定义的 `args`，再步骤的 `args`。 |

每个步骤只能有一个动作键。`biber`、`bibtex`、`makeindex` 的值必须是 `true`。`latex` 只接受 `pdflatex`、`xelatex` 和 `lualatex`。内置步骤可通过 `args` 追加参数，追加位置在 texmake 自己的参数之后。

## 变量

run 命令、步骤的 `args`、自定义工具的 `command` 和 `args` 可以使用以下变量：

| 变量 | 值 |
|---|---|
| `{root}` | 项目根的绝对路径。 |
| `{main}` | 主文档相对项目根的路径。 |
| `{main_stem}` | 主文档文件名去掉扩展名。 |
| `{output}` | 输出目录相对项目根的路径。 |
| `{name}` | 项目名。 |
| `{recipe}` | 当前配方名。 |

替换规则：

- 单次替换：替换后的值不再扫描。
- `{{` 和 `}}` 输出字面花括号。
- 其他 `{标识符}` 必须是以上 6 个变量之一，否则是配置错误。
- 其他花括号内容原样保留，例如 `awk '{print $1}'`。
- `${...}` 只允许出现在 run 步骤。其他步骤拒绝 `${` 和 `~`。
- 非 run 步骤不经过 shell，`$VAR` 原样传入。
- 变量替换不受引号影响：写在引号内也会替换。

## 自定义工具

`[tools]` 定义 tool 步骤使用的可执行文件：

```toml
[tools.graphviz]
command = "dot"
args = ["-Tpdf", "-o", "{output}/graph.pdf"]

[tools]
pandoc = "pandoc"
```

`command` 必需，`args` 可选。字符串简写等同于只写 `command`。步骤的 `args` 追加在工具定义的 `args` 之后。

命令路径的查找规则：命令含路径分隔符时，相对路径按项目根解析，绝对路径原样使用；其他命令在 `PATH` 中查找。工具在项目根运行。需要管道、重定向等 shell 语法时，使用 run 步骤。

## 跳过规则

同时满足以下条件时，texmake 跳过构建，并输出 `Everything is up to date.`：

- 配方包含 latex 步骤。
- 主 PDF 存在，且是普通文件。
- 主 PDF 的修改时间严格晚于项目根下所有普通文件的修改时间。

扫描源文件时，不跟随符号链接，忽略 `.git` 和所有配方的输出目录。配置文件（`texmake.toml`，或 `--file` 指定的文件）的修改时间也参与比较。

- `-B` 强制构建，忽略跳过判断。
- 不含 latex 步骤的配方每次都执行。
- 跳过构建时不创建输出目录，也不检查工具。
- `-q` 不输出 `Everything is up to date.`。

## 日志与失败报告

执行构建时，先清空 `<输出目录>/texmake.log`，再执行步骤。`-n` 不创建输出目录，也不清空日志；跳过构建时同样不清空。日志依次记录：

- 渲染后的命令，以 `$ ` 开头。
- 该步骤合并后的标准输出和标准错误，按到达顺序排列。
- 退出状态：`[退出码]` 或 `[signal 信号编号]`。

步骤执行失败时，texmake 停止构建。失败报告包含以下内容：

- 失败的命令。
- 退出码，或被信号终止的信号编号。
- 最后 20 行输出。
- 日志路径。

`-v` 实时输出工具输出；`-q` 仍输出失败报告。

缺少工具时，texmake 在执行前一次性列出全部缺失的可执行文件，退出码为 1。此时日志文件已创建，内容为空。工具检查覆盖 `latex`、`biber`、`bibtex`、`makeindex` 和 `tool` 步骤，不检查 `run` 步骤。

## 退出码

| 退出码 | 含义 |
|---|---|
| 0 | 成功。 |
| 1 | 构建失败，包括缺少工具。 |
| 2 | 配置错误、用法错误或环境错误。 |

## 基准测试

`cargo bench` 运行 Criterion 基准，覆盖三层：

- `vars`：变量渲染，含短模板、latex 命令行、原样花括号、长模板和未知变量。
- `config`：配置加载、校验、加载加校验、步骤展开。
- `scan`：2000 文件项目的跳过判断和最新源文件时间扫描。

基准只测进程内逻辑，不含进程启动和真实 TeX 编译。结果依赖机器和文件系统缓存，不作为 CI 门槛。HTML 报告在 `target/criterion/`。

## 0.1 限制

- 主文档必须是项目根下的 `.tex` 文件，不能位于子目录。
- 增量判断以配方为单位，配方内的步骤一起执行。
- 没有 watch 模式，配方之间不能共享公共步骤。
- 主 PDF 的文件名是主文档名去掉扩展名后加 `.pdf`。
- 支持 macOS 和 Linux；Windows 为尽力支持。
