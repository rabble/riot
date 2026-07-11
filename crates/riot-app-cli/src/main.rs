use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use riot_app_cli::{hex_lower, inspect, keygen, load_author, pack, write_new_atomic, PackInput};

fn main() {
    if let Err(error) = run(std::env::args().skip(1).collect()) {
        eprintln!("riot-app: {error}");
        std::process::exit(1);
    }
}

fn run(args: Vec<String>) -> Result<(), String> {
    let (command, rest) = args.split_first().ok_or_else(usage)?;
    match command.as_str() {
        "keygen" => run_keygen(rest),
        "pack" => run_pack(rest),
        "inspect" => run_inspect(rest),
        _ => Err(usage()),
    }
}

fn run_keygen(args: &[String]) -> Result<(), String> {
    let out = one_option(args, "--out")?;
    let generated = keygen(Path::new(&out)).map_err(|error| error.to_string())?;
    println!(
        "author signing key id: {}",
        hex_lower(&generated.identity.signing_key_id)
    );
    eprintln!("WARNING: {}", generated.warning);
    Ok(())
}

fn run_pack(args: &[String]) -> Result<(), String> {
    let (app_dir, options) = args.split_first().ok_or_else(usage)?;
    let mut key_dir = None;
    let mut out = None;
    let mut timestamp = None;
    let mut index = 0;
    while index < options.len() {
        let flag = &options[index];
        let value = options
            .get(index + 1)
            .ok_or_else(|| format!("missing value for {flag}"))?;
        match flag.as_str() {
            "--key-dir" if key_dir.is_none() => key_dir = Some(PathBuf::from(value)),
            "--out" if out.is_none() => out = Some(PathBuf::from(value)),
            "--timestamp-micros" if timestamp.is_none() => {
                timestamp = Some(value.parse::<u64>().map_err(|_| {
                    "--timestamp-micros must be an unsigned 64-bit integer".to_string()
                })?)
            }
            _ => {
                return Err(format!(
                    "unexpected or repeated argument '{flag}'\n{}",
                    usage()
                ))
            }
        }
        index += 2;
    }
    let key_dir = key_dir.ok_or_else(|| "pack requires --key-dir <dir>".to_string())?;
    let out = out.ok_or_else(|| "pack requires --out <file>".to_string())?;
    let timestamp_micros = match timestamp {
        Some(value) => value,
        None => current_unix_micros()?,
    };
    let author = load_author(&key_dir).map_err(|error| error.to_string())?;
    let output = pack(PackInput {
        app_dir: Path::new(app_dir),
        author: &author,
        timestamp_micros,
    })
    .map_err(|error| error.to_string())?;
    write_new_atomic(&out, &output.import_bundle_bytes).map_err(|error| error.to_string())?;
    println!("app id: {}", hex_lower(&output.app_id));
    println!("wrote: {}", out.display());
    Ok(())
}

fn run_inspect(args: &[String]) -> Result<(), String> {
    if args.len() != 1 {
        return Err("inspect requires exactly one bundle file".into());
    }
    let path = Path::new(&args[0]);
    let bytes =
        std::fs::read(path).map_err(|error| format!("read '{}': {error}", path.display()))?;
    let report = inspect(&bytes).map_err(|error| error.to_string())?;
    println!("name: {}", report.name);
    println!("version: {}", report.version);
    println!(
        "author signing key id: {}",
        hex_lower(&report.author.signing_key_id)
    );
    println!("app id: {}", hex_lower(&report.app_id));
    println!("resources:");
    for resource in report.resources {
        println!("  {resource}");
    }
    Ok(())
}

fn one_option(args: &[String], expected: &str) -> Result<String, String> {
    if args.len() == 2 && args[0] == expected {
        Ok(args[1].clone())
    } else {
        Err(usage())
    }
}

fn current_unix_micros() -> Result<u64, String> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "system clock is before the Unix epoch".to_string())?;
    u64::try_from(duration.as_micros())
        .map_err(|_| "current Unix timestamp does not fit in u64 microseconds".to_string())
}

fn usage() -> String {
    "usage:\n  riot-app keygen --out <dir>\n  riot-app pack <app-dir> --key-dir <dir> --out <file> [--timestamp-micros <u64>]\n  riot-app inspect <file>".into()
}
