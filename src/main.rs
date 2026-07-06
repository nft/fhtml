use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::exit;
use std::{env, fs, io};

use fhtml::{compile_opts, format, Mode, Options};

const USAGE: &str = "\
fhtml — compiler for Fluid HTML (see SPEC.md)

USAGE:
  fhtml [OPTIONS] [FILE]           compile FILE (or stdin) to stdout
  fhtml build <SRC> [-o <PATH>]    compile a .fhtml file or directory tree
  fhtml fmt [FILE|DIR]             reformat to canonical style (in place;
                                   stdin prints to stdout)

OPTIONS:
  -o <PATH>      output file, or output directory for `build` of a directory
                 (default: dist)
  --pretty       indented output (default when writing files)
  --min          minified output (default when writing to stdout)
  --no-templates enforce static markup (SPEC §9.2): any template construct —
                 statements, `{…}` interpolation, unescaped `{` — is an error
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
    let mut templates = true;
    let mut input: Option<String> = None;

    let args: Vec<String> = env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--pretty" => pretty = Some(true),
            "--min" => pretty = Some(false),
            "--no-templates" => templates = false,
            "-o" => {
                i += 1;
                let val = args.get(i).ok_or("`-o` requires a path")?;
                out_path = Some(PathBuf::from(val));
            }
            "-h" | "--help" => {
                print!("{USAGE}");
                return Ok(());
            }
            "-V" | "--version" => {
                println!("fhtml {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            "build" if !build && !fmt && input.is_none() => build = true,
            "fmt" if !build && !fmt && input.is_none() => fmt = true,
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

    if build {
        let src = input.ok_or("`fhtml build` requires a source path")?;
        let src = PathBuf::from(src);
        if src.is_dir() {
            build_dir(
                &src,
                &out_path.unwrap_or_else(|| PathBuf::from("dist")),
                pretty,
                templates,
            )
        } else {
            let out = out_path.unwrap_or_else(|| src.with_extension("html"));
            build_file(&src, &out, pretty, templates)
        }
    } else if fmt {
        run_fmt(input.as_deref(), out_path)
    } else {
        let (name, source) = match input.as_deref() {
            None | Some("-") => ("<stdin>".to_string(), read_stdin()?),
            Some(path) => (
                path.to_string(),
                fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?,
            ),
        };
        // SPEC §11: pretty when writing files, min for pipelines/stdout.
        let mode = mode_for(pretty, out_path.is_some());
        let opts = Options { mode, templates };
        let output = compile_opts(&source, &opts).map_err(|e| format!("{name}:{e}"))?;
        print_warnings(&name, &output.warnings);
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

fn run_fmt(input: Option<&str>, out_path: Option<PathBuf>) -> Result<(), String> {
    match input {
        None | Some("-") => {
            let formatted = format(&read_stdin()?).map_err(|e| format!("<stdin>:{e}"))?;
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
                    if fmt_file(file, file)? {
                        changed += 1;
                    }
                }
                eprintln!("formatted {changed}/{} file(s)", files.len());
                Ok(())
            } else {
                let out = out_path.unwrap_or_else(|| path.clone());
                fmt_file(&path, &out)?;
                Ok(())
            }
        }
    }
}

/// Returns whether the file's contents changed.
fn fmt_file(src: &Path, out: &Path) -> Result<bool, String> {
    let source = fs::read_to_string(src).map_err(|e| format!("{}: {e}", src.display()))?;
    let formatted = format(&source).map_err(|e| format!("{}:{e}", src.display()))?;
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

fn build_file(src: &Path, out: &Path, pretty: Option<bool>, templates: bool) -> Result<(), String> {
    let source = fs::read_to_string(src).map_err(|e| format!("{}: {e}", src.display()))?;
    let opts = Options {
        mode: mode_for(pretty, true),
        templates,
    };
    let output = compile_opts(&source, &opts).map_err(|e| format!("{}:{e}", src.display()))?;
    print_warnings(&src.display().to_string(), &output.warnings);
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    fs::write(out, output.html).map_err(|e| format!("{}: {e}", out.display()))
}

fn build_dir(
    src: &Path,
    out_dir: &Path,
    pretty: Option<bool>,
    templates: bool,
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
        let out = out_dir.join(rel).with_extension("html");
        if let Err(msg) = build_file(file, &out, pretty, templates) {
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
