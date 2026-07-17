use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::exit;
use std::{env, fs, io};

use fhtml::{
    compile_opts, compile_to_js_opts_from, format_shorthand, render_opts_from, FmtShorthand, Mode,
    Options, ShorthandPolicy, Value,
};

const USAGE: &str = "\
fhtml — compiler for Fluid HTML (see SPEC.md)

USAGE:
  fhtml [OPTIONS] [FILE]           compile/render FILE (or stdin) to stdout
  fhtml build <SRC> [-o <PATH>]    compile a .fhtml file or directory tree
  fhtml fmt [FILE|DIR]             reformat to canonical style (in place;
                                   stdin prints to stdout)
  fhtml deps <FILE>                list every transitively included file
                                   (absolute paths, one per line; empty if
                                   none) — the watch set for HMR/CI

OPTIONS:
  -o <PATH>      output file, or output directory for `build` of a directory
                 (default: dist)
  --pretty       indented output (default when writing files)
  --min          minified output (default when writing to stdout)
  --data <FILE>  JSON data for the template layer (SPEC §9–§10); harmless on
                 template-free files. Without it, template files render with
                 every name null
  --ctx <FILE>   JSON bound to the read-only `ctx` root (SPEC §9.4)
  --target=js    emit a self-contained ES module exporting
                 `(data, ctx = {}) => string` instead of HTML; `build` writes
                 `.js` files in the same tree layout. Static files become a
                 constant function, for uniformity
  --no-templates enforce static markup (SPEC §9.2): any template construct —
                 statements, `{…}` interpolation, unescaped `{` — is an error
  --shorthand    decode class shorthand (SPEC §3.2) in every file, directive
                 or not
  --no-shorthand never decode class shorthand, even under a `#!shorthand`
                 directive (`=` escapes stay literal too)
  --deny-warnings
                 exit non-zero if any warning was emitted (warnings still
                 print; output files are not written) — for CI
  --contract     with `fmt`: rewrite classes into shorthand form — codes where
                 they round-trip, `#!shorthand` directive added (SPEC §3.2)
  --expand       with `fmt`: rewrite shorthand back to full classes and drop
                 the directive; compiled output is unchanged either way
  -h, --help     show this help
  -V, --version  print version
";

fn main() {
    match run() {
        Ok(()) => {}
        Err(msg) => {
            eprintln!("{msg}");
            exit(1);
        }
    }
}

fn run() -> Result<(), String> {
    let mut pretty: Option<bool> = None;
    let mut out_path: Option<PathBuf> = None;
    let mut build = false;
    let mut fmt = false;
    let mut deps = false;
    let mut templates = true;
    let mut shorthand: Option<ShorthandPolicy> = None;
    let mut fmt_shorthand: Option<FmtShorthand> = None;
    let mut js_target = false;
    let mut data_path: Option<PathBuf> = None;
    let mut ctx_path: Option<PathBuf> = None;
    let mut deny_warnings = false;
    let mut input: Option<String> = None;

    let args: Vec<String> = env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--pretty" => pretty = Some(true),
            "--min" => pretty = Some(false),
            "--no-templates" => templates = false,
            s @ ("--shorthand" | "--no-shorthand") => {
                let p = if s == "--shorthand" {
                    ShorthandPolicy::On
                } else {
                    ShorthandPolicy::Off
                };
                if shorthand.is_some_and(|prev| prev != p) {
                    return Err(
                        "`--shorthand` and `--no-shorthand` are mutually exclusive".to_string()
                    );
                }
                shorthand = Some(p);
            }
            s @ ("--contract" | "--expand") => {
                let m = if s == "--contract" {
                    FmtShorthand::Contract
                } else {
                    FmtShorthand::Expand
                };
                if fmt_shorthand.is_some_and(|prev| prev != m) {
                    return Err("`--contract` and `--expand` are mutually exclusive".to_string());
                }
                fmt_shorthand = Some(m);
            }
            "--deny-warnings" => deny_warnings = true,
            "--target=js" => js_target = true,
            "--target=html" => js_target = false,
            "--target" => {
                i += 1;
                match args.get(i).map(String::as_str) {
                    Some("js") => js_target = true,
                    Some("html") => js_target = false,
                    other => {
                        return Err(format!(
                            "`--target` takes `js` or `html`, got {}",
                            other.map_or("nothing".to_string(), |t| format!("`{t}`"))
                        ))
                    }
                }
            }
            "-o" => {
                i += 1;
                let val = args.get(i).ok_or("`-o` requires a path")?;
                out_path = Some(PathBuf::from(val));
            }
            "--data" => {
                i += 1;
                let val = args.get(i).ok_or("`--data` requires a JSON file path")?;
                data_path = Some(PathBuf::from(val));
            }
            "--ctx" => {
                i += 1;
                let val = args.get(i).ok_or("`--ctx` requires a JSON file path")?;
                ctx_path = Some(PathBuf::from(val));
            }
            "-h" | "--help" => {
                print!("{USAGE}");
                return Ok(());
            }
            "-V" | "--version" => {
                println!("fhtml {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            "build" if !build && !fmt && !deps && input.is_none() => build = true,
            "fmt" if !build && !fmt && !deps && input.is_none() => fmt = true,
            "deps" if !build && !fmt && !deps && input.is_none() => deps = true,
            s if s.starts_with('-') && s != "-" => {
                return Err(format!("unknown option `{s}` (see `fhtml --help`)"))
            }
            s => {
                if input.is_some() {
                    return Err(format!("unexpected argument `{s}`"));
                }
                input = Some(s.to_string());
            }
        }
        i += 1;
    }

    if !templates && (data_path.is_some() || ctx_path.is_some()) {
        return Err("`--data`/`--ctx` cannot be combined with `--no-templates`".to_string());
    }
    if js_target && (data_path.is_some() || ctx_path.is_some()) {
        return Err(
            "`--data`/`--ctx` cannot be combined with `--target=js` — the emitted module takes \
             data at call time: `render(data, ctx)`"
                .to_string(),
        );
    }
    if fmt && shorthand.is_some() {
        return Err(
            "`fmt` always preserves the authored form — `--shorthand`/`--no-shorthand` do not \
             apply; `fmt --contract`/`--expand` rewrite between the forms (SPEC §3.2)"
                .to_string(),
        );
    }
    if !fmt && fmt_shorthand.is_some() {
        return Err(
            "`--contract`/`--expand` only apply to `fmt`; compiling takes \
             `--shorthand`/`--no-shorthand` (SPEC §3.2)"
                .to_string(),
        );
    }
    let data = load_json(data_path.as_deref())?;
    let ctx = load_json(ctx_path.as_deref())?;
    let job = Job {
        templates,
        shorthand: shorthand.unwrap_or_default(),
        js_target,
        data,
        ctx,
    };

    if build {
        let src = input.ok_or("`fhtml build` requires a source path")?;
        let src = PathBuf::from(src);
        let ext = if js_target { "js" } else { "html" };
        if src.is_dir() {
            build_dir(
                &src,
                &out_path.unwrap_or_else(|| PathBuf::from("dist")),
                pretty,
                &job,
                deny_warnings,
            )
        } else {
            let out = out_path.unwrap_or_else(|| src.with_extension(ext));
            build_file(&src, &out, pretty, &job, deny_warnings)
        }
    } else if fmt {
        run_fmt(
            input.as_deref(),
            out_path,
            fmt_shorthand.unwrap_or_default(),
        )
    } else if deps {
        // Includes are relative to the including file (SPEC §10.5), so this
        // needs a real path — no stdin form.
        let path = input.ok_or("`fhtml deps` requires a file path")?;
        let source = fs::read_to_string(&path).map_err(|e| format!("{path}: {e}"))?;
        let list =
            fhtml::deps_from(&source, Some(Path::new(&path))).map_err(|e| format!("{path}:{e}"))?;
        for p in &list {
            println!("{}", p.display());
        }
        Ok(())
    } else {
        let (name, source) = match input.as_deref() {
            None | Some("-") => ("<stdin>".to_string(), read_stdin()?),
            Some(path) => (
                path.to_string(),
                fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?,
            ),
        };
        let file = match input.as_deref() {
            None | Some("-") => None,
            Some(path) => Some(PathBuf::from(path)),
        };
        // SPEC §11: pretty when writing files, min for pipelines/stdout.
        let mode = mode_for(pretty, out_path.is_some());
        let output = job
            .run(&source, file.as_deref(), mode)
            .map_err(|e| format!("{name}:{e}"))?;
        print_warnings(&name, &output.warnings);
        deny(deny_warnings, &output.warnings)?;
        match out_path {
            Some(path) => {
                fs::write(&path, output.html).map_err(|e| format!("{}: {e}", path.display()))
            }
            None => {
                print!("{}", output.html);
                Ok(())
            }
        }
    }
}

/// What to do with each source file: render with data (the default), emit a
/// JS module with `--target=js`, or static-enforce with `--no-templates`.
struct Job {
    templates: bool,
    shorthand: ShorthandPolicy,
    js_target: bool,
    data: Value,
    ctx: Value,
}

impl Job {
    /// `file` is the path the source was read from — the base for resolving
    /// `include` (SPEC §10.5). `None` for stdin, where includes are an error.
    fn run(
        &self,
        source: &str,
        file: Option<&Path>,
        mode: Mode,
    ) -> Result<fhtml::Output, fhtml::Error> {
        let opts = Options {
            mode,
            templates: self.templates,
            shorthand: self.shorthand,
        };
        if self.js_target {
            compile_to_js_opts_from(source, file, &opts)
        } else if self.templates {
            render_opts_from(source, file, &self.data, &self.ctx, &opts)
        } else {
            compile_opts(source, &opts)
        }
    }
}

fn load_json(path: Option<&Path>) -> Result<Value, String> {
    match path {
        None => Ok(Value::Null),
        Some(p) => {
            let text = fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))?;
            fhtml::json::parse(&text).map_err(|e| format!("{}:{e}", p.display()))
        }
    }
}

fn read_stdin() -> Result<String, String> {
    let mut buf = String::new();
    io::stdin()
        .read_to_string(&mut buf)
        .map_err(|e| format!("failed to read stdin: {e}"))?;
    Ok(buf)
}

fn print_warnings(name: &str, warnings: &[String]) {
    for w in warnings {
        eprintln!("{name}:{w}");
    }
}

/// `--deny-warnings`: any warning fails the run (after printing it), and the
/// pending output is not written — CI semantics, like rustc's `-Dwarnings`.
fn deny(deny_warnings: bool, warnings: &[String]) -> Result<(), String> {
    if deny_warnings && !warnings.is_empty() {
        return Err(format!(
            "{} warning(s) denied (--deny-warnings)",
            warnings.len()
        ));
    }
    Ok(())
}

fn run_fmt(
    input: Option<&str>,
    out_path: Option<PathBuf>,
    shorthand: FmtShorthand,
) -> Result<(), String> {
    match input {
        None | Some("-") => {
            let formatted =
                format_shorthand(&read_stdin()?, shorthand).map_err(|e| format!("<stdin>:{e}"))?;
            print!("{formatted}");
            Ok(())
        }
        Some(path) => {
            let path = PathBuf::from(path);
            if path.is_dir() {
                let mut files = Vec::new();
                collect_fhtml(&path, &mut files).map_err(|e| format!("{}: {e}", path.display()))?;
                if files.is_empty() {
                    return Err(format!("no .fhtml files found under {}", path.display()));
                }
                files.sort();
                let mut changed = 0usize;
                for file in &files {
                    if fmt_file(file, file, shorthand)? {
                        changed += 1;
                    }
                }
                eprintln!("formatted {changed}/{} file(s)", files.len());
                Ok(())
            } else {
                let out = out_path.unwrap_or_else(|| path.clone());
                fmt_file(&path, &out, shorthand)?;
                Ok(())
            }
        }
    }
}

/// Returns whether the file's contents changed.
fn fmt_file(src: &Path, out: &Path, shorthand: FmtShorthand) -> Result<bool, String> {
    let source = fs::read_to_string(src).map_err(|e| format!("{}: {e}", src.display()))?;
    let formatted =
        format_shorthand(&source, shorthand).map_err(|e| format!("{}:{e}", src.display()))?;
    if src == out && formatted == source {
        return Ok(false);
    }
    fs::write(out, &formatted).map_err(|e| format!("{}: {e}", out.display()))?;
    Ok(formatted != source)
}

fn mode_for(pretty: Option<bool>, writing_file: bool) -> Mode {
    match pretty.unwrap_or(writing_file) {
        true => Mode::Pretty,
        false => Mode::Min,
    }
}

fn build_file(
    src: &Path,
    out: &Path,
    pretty: Option<bool>,
    job: &Job,
    deny_warnings: bool,
) -> Result<(), String> {
    let source = fs::read_to_string(src).map_err(|e| format!("{}: {e}", src.display()))?;
    let output = job
        .run(&source, Some(src), mode_for(pretty, true))
        .map_err(|e| format!("{}:{e}", src.display()))?;
    print_warnings(&src.display().to_string(), &output.warnings);
    deny(deny_warnings, &output.warnings).map_err(|e| format!("{}: {e}", src.display()))?;
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    fs::write(out, output.html).map_err(|e| format!("{}: {e}", out.display()))
}

fn build_dir(
    src: &Path,
    out_dir: &Path,
    pretty: Option<bool>,
    job: &Job,
    deny_warnings: bool,
) -> Result<(), String> {
    let mut files = Vec::new();
    collect_fhtml(src, &mut files).map_err(|e| format!("{}: {e}", src.display()))?;
    if files.is_empty() {
        return Err(format!("no .fhtml files found under {}", src.display()));
    }
    files.sort();

    let mut failures = 0usize;
    for file in &files {
        let rel = file.strip_prefix(src).unwrap();
        let ext = if job.js_target { "js" } else { "html" };
        let out = out_dir.join(rel).with_extension(ext);
        if let Err(msg) = build_file(file, &out, pretty, job, deny_warnings) {
            eprintln!("{msg}");
            failures += 1;
        }
    }
    let ok = files.len() - failures;
    eprintln!(
        "compiled {ok}/{} file(s) → {}",
        files.len(),
        out_dir.display()
    );
    if failures > 0 {
        Err(format!("{failures} file(s) failed"))
    } else {
        Ok(())
    }
}

fn collect_fhtml(dir: &Path, out: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect_fhtml(&path, out)?;
        } else if path.extension().is_some_and(|e| e == "fhtml") {
            out.push(path);
        }
    }
    Ok(())
}
