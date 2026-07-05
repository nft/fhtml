use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::exit;
use std::{env, fs, io};

use fhtml::convert::{check, compare_html, convert, Options};

const USAGE: &str = "\
html2fhtml — convert HTML to fhtml

USAGE:
  html2fhtml [OPTIONS] [FILE]      convert FILE (or stdin) to stdout
  html2fhtml <DIR> -o <DIR>        convert a tree of .html → .fhtml

OPTIONS:
  -o <PATH>          output file, or output directory for a directory input
  --convert-svg      convert svg/math subtrees instead of raw passthrough
  --no-chains        disable `>` chain synthesis
  --fragment[=TAG]   parse input as a fragment (context element TAG,
                     default body) — for snippets that document parsing
                     mangles, e.g. a bare <tr>
  --check            convert, recompile with fhtml, compare normalized
                     DOMs; exit 1 on mismatch
  --dom-eq <A> <B>   compare two HTML files for normalized-DOM
                     equivalence (no conversion); exit 1 and describe
                     the first difference on mismatch
  -h, --help         show this help
  -V, --version      print version

Whitespace between elements is treated as non-contractual (collapsed or
dropped), matching fhtml's own contract; `white-space: pre` applied via CSS
to arbitrary elements is invisible to a markup-only tool. <pre> and
<textarea> are preserved byte-exactly.
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
    let mut opts = Options::default();
    let mut out_path: Option<PathBuf> = None;
    let mut do_check = false;
    let mut dom_eq: Option<(String, String)> = None;
    let mut input: Option<String> = None;

    let args: Vec<String> = env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--convert-svg" => opts.convert_svg = true,
            "--no-chains" => opts.chains = false,
            "--check" => do_check = true,
            "--dom-eq" => {
                let a = args
                    .get(i + 1)
                    .ok_or("`--dom-eq` requires two file paths")?;
                let b = args
                    .get(i + 2)
                    .ok_or("`--dom-eq` requires two file paths")?;
                dom_eq = Some((a.clone(), b.clone()));
                i += 2;
            }
            "--fragment" => opts.fragment = Some("body".to_string()),
            s if s.starts_with("--fragment=") => {
                let ctx = &s["--fragment=".len()..];
                if ctx.is_empty() {
                    return Err("`--fragment=` requires a context element name".to_string());
                }
                opts.fragment = Some(ctx.to_string());
            }
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
                println!("html2fhtml {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            s if s.starts_with('-') && s != "-" => {
                return Err(format!("unknown option `{s}` (see `html2fhtml --help`)"))
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

    if let Some((a, b)) = dom_eq {
        if input.is_some() || out_path.is_some() || do_check {
            return Err("`--dom-eq` takes exactly two files and no other input".to_string());
        }
        let read = |p: &str| fs::read_to_string(p).map_err(|e| format!("{p}: {e}"));
        return compare_html(&read(&a)?, &read(&b)?, &opts)
            .map_err(|e| format!("DOM mismatch\n  {e}"));
    }

    if let Some(path) = input.as_deref().map(Path::new) {
        if path.is_dir() {
            return convert_dir(
                path,
                out_path.as_deref().ok_or(
                    "converting a directory requires `-o <DIR>` (see `html2fhtml --help`)",
                )?,
                &opts,
                do_check,
            );
        }
    }

    let (name, source) = match input.as_deref() {
        None | Some("-") => ("<stdin>".to_string(), read_stdin()?),
        Some(path) => (
            path.to_string(),
            fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?,
        ),
    };
    let fhtml_src = convert_one(&name, &source, &opts, do_check)?;
    match out_path {
        Some(path) => fs::write(&path, fhtml_src).map_err(|e| format!("{}: {e}", path.display())),
        None => {
            print!("{fhtml_src}");
            Ok(())
        }
    }
}

/// Converts one source, printing warnings to stderr; with `check` the
/// round-trip is verified and a mismatch is an error.
fn convert_one(name: &str, source: &str, opts: &Options, do_check: bool) -> Result<String, String> {
    if do_check {
        let out = convert(source, opts);
        for w in &out.warnings {
            eprintln!("{name}: warning: {w}");
        }
        check(source, opts).map_err(|e| format!("{name}: round-trip mismatch\n  {e}"))
    } else {
        let out = convert(source, opts);
        for w in &out.warnings {
            eprintln!("{name}: warning: {w}");
        }
        Ok(out.fhtml)
    }
}

fn convert_dir(src: &Path, out_dir: &Path, opts: &Options, do_check: bool) -> Result<(), String> {
    let mut files = Vec::new();
    collect_html(src, &mut files).map_err(|e| format!("{}: {e}", src.display()))?;
    if files.is_empty() {
        return Err(format!("no .html files found under {}", src.display()));
    }
    files.sort();

    let mut failures = 0usize;
    for file in &files {
        let rel = file.strip_prefix(src).unwrap();
        let out = out_dir.join(rel).with_extension("fhtml");
        let result = fs::read_to_string(file)
            .map_err(|e| format!("{}: {e}", file.display()))
            .and_then(|source| convert_one(&file.display().to_string(), &source, opts, do_check))
            .and_then(|fhtml_src| {
                if let Some(parent) = out.parent() {
                    fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
                }
                fs::write(&out, fhtml_src).map_err(|e| format!("{}: {e}", out.display()))
            });
        if let Err(msg) = result {
            eprintln!("{msg}");
            failures += 1;
        }
    }
    let ok = files.len() - failures;
    eprintln!(
        "converted {ok}/{} file(s) → {}",
        files.len(),
        out_dir.display()
    );
    if failures > 0 {
        Err(format!("{failures} file(s) failed"))
    } else {
        Ok(())
    }
}

fn collect_html(dir: &Path, out: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect_html(&path, out)?;
        } else if path.extension().is_some_and(|e| e == "html" || e == "htm") {
            out.push(path);
        }
    }
    Ok(())
}

fn read_stdin() -> Result<String, String> {
    let mut buf = String::new();
    io::stdin()
        .read_to_string(&mut buf)
        .map_err(|e| format!("failed to read stdin: {e}"))?;
    Ok(buf)
}
