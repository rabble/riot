use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use riot_app_cli::{hex_lower, inspect, keygen, load_author, pack, write_new_atomic, PackInput};

fn main() -> ExitCode {
    let mut stdout = std::io::stdout().lock();
    let mut stderr = std::io::stderr().lock();
    main_with(
        std::env::args_os().skip(1).collect(),
        &SystemClock,
        &mut stdout,
        &mut stderr,
    )
}

trait Clock {
    fn unix_micros(&self) -> Result<u64, String>;
}

impl<F> Clock for F
where
    F: Fn() -> Result<u64, String>,
{
    fn unix_micros(&self) -> Result<u64, String> {
        self()
    }
}

struct SystemClock;

impl Clock for SystemClock {
    fn unix_micros(&self) -> Result<u64, String> {
        unix_micros_at(SystemTime::now())
    }
}

fn main_with(
    args: Vec<OsString>,
    clock: &dyn Clock,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> ExitCode {
    let result = args
        .into_iter()
        .map(|value| {
            value
                .into_string()
                .map_err(|_| "arguments must be valid UTF-8".to_string())
        })
        .collect::<Result<Vec<_>, _>>()
        .and_then(|args| run(args, clock, stdout, stderr));
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let _ = writeln!(stderr, "riot-app: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(
    args: Vec<String>,
    clock: &dyn Clock,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Result<(), String> {
    let (command, rest) = args.split_first().ok_or_else(usage)?;
    match command.as_str() {
        "keygen" => run_keygen(rest, stdout, stderr),
        "pack" => run_pack(rest, clock, stdout),
        "inspect" => run_inspect(rest, stdout),
        _ => Err(usage()),
    }
}

fn run_keygen(
    args: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Result<(), String> {
    let out = one_option(args, "--out")?;
    let generated = keygen(Path::new(&out)).map_err(|error| error.to_string())?;
    writeln!(
        stdout,
        "author signing key id: {}",
        hex_lower(&generated.identity.signing_key_id)
    )
    .map_err(|error| format!("write output: {error}"))?;
    writeln!(stderr, "WARNING: {}", generated.warning)
        .map_err(|error| format!("write warning: {error}"))?;
    Ok(())
}

fn run_pack(args: &[String], clock: &dyn Clock, stdout: &mut dyn Write) -> Result<(), String> {
    let (app_dir, options) = args.split_first().ok_or_else(usage)?;
    let mut key_dir = None;
    let mut out = None;
    let mut timestamp = None;
    let mut index = 0;
    while index < options.len() {
        let flag = &options[index];
        let value = options
            .get(index + 1)
            .ok_or_else(|| format!("missing value for {}", escaped(flag)))?;
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
                    "unexpected or repeated argument '{}'\n{}",
                    escaped(flag),
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
        None => clock.unix_micros()?,
    };
    let author = load_author(&key_dir).map_err(|error| error.to_string())?;
    let output = pack(PackInput {
        app_dir: Path::new(app_dir),
        author: &author,
        timestamp_micros,
    })
    .map_err(|error| error.to_string())?;
    write_new_atomic(&out, &output.import_bundle_bytes).map_err(|error| error.to_string())?;
    writeln!(stdout, "app id: {}", hex_lower(&output.app_id))
        .and_then(|()| writeln!(stdout, "wrote: {}", escaped(&out.to_string_lossy())))
        .map_err(|error| format!("write output: {error}"))?;
    Ok(())
}

fn run_inspect(args: &[String], stdout: &mut dyn Write) -> Result<(), String> {
    run_inspect_with(args, stdout, &OsInspectInput)
}

trait InspectInput {
    fn metadata(&self, path: &Path) -> std::io::Result<std::fs::Metadata>;
    fn open(&self, path: &Path) -> std::io::Result<std::fs::File>;
    fn read(
        &self,
        file: &mut std::fs::File,
        limit: u64,
        bytes: &mut Vec<u8>,
    ) -> std::io::Result<usize>;
}

struct OsInspectInput;

impl InspectInput for OsInspectInput {
    fn metadata(&self, path: &Path) -> std::io::Result<std::fs::Metadata> {
        std::fs::metadata(path)
    }

    fn open(&self, path: &Path) -> std::io::Result<std::fs::File> {
        std::fs::File::open(path)
    }

    fn read(
        &self,
        file: &mut std::fs::File,
        limit: u64,
        bytes: &mut Vec<u8>,
    ) -> std::io::Result<usize> {
        Read::by_ref(file).take(limit).read_to_end(bytes)
    }
}

fn run_inspect_with(
    args: &[String],
    stdout: &mut dyn Write,
    input: &dyn InspectInput,
) -> Result<(), String> {
    if args.len() != 1 {
        return Err("inspect requires exactly one bundle file".into());
    }
    let path = Path::new(&args[0]);
    let metadata = input
        .metadata(path)
        .map_err(|error| format!("read input metadata: {error}"))?;
    let limit = riot_core::import::MAX_BUNDLE_BYTES;
    ensure_input_size(metadata.len(), limit)?;
    let mut file = input
        .open(path)
        .map_err(|error| format!("open input: {error}"))?;
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    input
        .read(&mut file, (limit as u64) + 1, &mut bytes)
        .map_err(|error| format!("read input: {error}"))?;
    ensure_input_size(bytes.len() as u64, limit)?;
    let report = inspect(&bytes).map_err(|error| error.to_string())?;
    writeln!(stdout, "name: {}", report.name)
        .and_then(|()| writeln!(stdout, "version: {}", report.version))
        .and_then(|()| {
            writeln!(
                stdout,
                "author signing key id: {}",
                hex_lower(&report.author.signing_key_id)
            )
        })
        .and_then(|()| writeln!(stdout, "app id: {}", hex_lower(&report.app_id)))
        .and_then(|()| writeln!(stdout, "resources:"))
        .map_err(|error| format!("write output: {error}"))?;
    for resource in report.resources {
        writeln!(stdout, "  {resource}").map_err(|error| format!("write output: {error}"))?;
    }
    Ok(())
}

fn ensure_input_size(actual: u64, limit: usize) -> Result<(), String> {
    if actual > limit as u64 {
        Err(format!(
            "input is too large: {actual} bytes (limit {limit} bytes)"
        ))
    } else {
        Ok(())
    }
}

fn one_option(args: &[String], expected: &str) -> Result<String, String> {
    if args.len() == 2 && args[0] == expected {
        Ok(args[1].clone())
    } else {
        Err(usage())
    }
}

fn unix_micros_at(now: SystemTime) -> Result<u64, String> {
    let duration = now
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "system clock is before the Unix epoch".to_string())?;
    duration_to_micros(duration)
}

fn duration_to_micros(duration: std::time::Duration) -> Result<u64, String> {
    u64::try_from(duration.as_micros())
        .map_err(|_| "current Unix timestamp does not fit in u64 microseconds".to_string())
}

fn usage() -> String {
    "riot-app currently supports Unix only\nusage:\n  riot-app keygen --out <dir>\n  riot-app pack <app-dir> --key-dir <dir> --out <file> [--timestamp-micros <u64>]\n  riot-app inspect <file>".into()
}

fn escaped(value: &str) -> String {
    value.chars().flat_map(char::escape_default).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    struct FailingWriter;

    impl Write for FailingWriter {
        fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "injected broken pipe",
            ))
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "injected broken pipe",
            ))
        }
    }

    struct FailAfterLines {
        lines: usize,
        limit: usize,
    }

    struct ScriptedInput {
        fail_open: bool,
        oversized_read: bool,
    }

    impl InspectInput for ScriptedInput {
        fn metadata(&self, path: &Path) -> std::io::Result<std::fs::Metadata> {
            std::fs::metadata(path)
        }

        fn open(&self, path: &Path) -> std::io::Result<std::fs::File> {
            if self.fail_open {
                Err(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "injected open failure",
                ))
            } else {
                std::fs::File::open(path)
            }
        }

        fn read(
            &self,
            file: &mut std::fs::File,
            limit: u64,
            bytes: &mut Vec<u8>,
        ) -> std::io::Result<usize> {
            if self.oversized_read {
                bytes.resize(limit as usize, 0);
                Ok(bytes.len())
            } else {
                OsInspectInput.read(file, limit, bytes)
            }
        }
    }

    impl Write for FailAfterLines {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            if self.lines >= self.limit {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::BrokenPipe,
                    "injected line limit",
                ));
            }
            self.lines += buf.iter().filter(|byte| **byte == b'\n').count();
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn invoke(args: &[&str], clock: &dyn Clock) -> (ExitCode, String, String) {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let status = main_with(
            args.iter().map(OsString::from).collect(),
            clock,
            &mut stdout,
            &mut stderr,
        );
        (
            status,
            String::from_utf8(stdout).unwrap(),
            String::from_utf8(stderr).unwrap(),
        )
    }

    #[test]
    fn main_with_reports_dispatch_and_clock_errors_without_exiting() {
        let clock = || Ok(123_u64);
        for args in [&[][..], &["unknown"][..], &["keygen", "--out"][..]] {
            let (status, stdout, stderr) = invoke(args, &clock);
            assert_eq!(status, ExitCode::FAILURE);
            assert!(stdout.is_empty());
            assert!(stderr.starts_with("riot-app: "));
        }

        let failing_clock = || Err("system clock is before the Unix epoch".to_string());
        let (status, _, stderr) = invoke(
            &["pack", "app", "--key-dir", "keys", "--out", "bundle.riot"],
            &failing_clock,
        );
        assert_eq!(status, ExitCode::FAILURE);
        assert!(stderr.contains("system clock is before the Unix epoch"));

        let result = main();
        assert!([ExitCode::FAILURE, ExitCode::SUCCESS].contains(&result));
        let (status, _, stderr) = invoke(&["keygen", "--wrong", "value"], &clock);
        assert_eq!(status, ExitCode::FAILURE);
        assert!(stderr.contains("usage:"));

        let mut failing = FailingWriter;
        assert_eq!(
            failing.flush().unwrap_err().kind(),
            std::io::ErrorKind::BrokenPipe
        );
        let mut line_writer = FailAfterLines { lines: 0, limit: 1 };
        line_writer.flush().unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn main_with_rejects_non_utf8_arguments() {
        use std::os::unix::ffi::OsStringExt;

        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let status = main_with(
            vec![OsString::from_vec(vec![0xff])],
            &|| Ok(1),
            &mut stdout,
            &mut stderr,
        );
        assert_eq!(status, ExitCode::FAILURE);
        assert!(stdout.is_empty());
        assert_eq!(
            String::from_utf8(stderr).unwrap(),
            "riot-app: arguments must be valid UTF-8\n"
        );
    }

    #[test]
    fn main_with_drives_keygen_pack_and_inspect_with_injected_io() {
        let temp = tempfile::tempdir_in(env!("CARGO_MANIFEST_DIR")).unwrap();
        let keys = temp.path().join("keys");
        let artifact = temp.path().join("bundle\nname.riot");
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/hello-app");
        let clock = || Ok(77_u64);

        let (status, stdout, stderr) = invoke(&["keygen", "--out", keys.to_str().unwrap()], &clock);
        assert_eq!(status, ExitCode::SUCCESS);
        assert!(stdout.starts_with("author signing key id: "));
        assert!(stderr.contains("WARNING: Protect both author.wrapkey"));

        let (status, stdout, stderr) = invoke(
            &[
                "pack",
                fixture.to_str().unwrap(),
                "--key-dir",
                keys.to_str().unwrap(),
                "--out",
                artifact.to_str().unwrap(),
            ],
            &clock,
        );
        assert_eq!(status, ExitCode::SUCCESS, "{stderr}");
        assert!(stdout.contains("app id: "));
        assert!(stdout.contains("bundle\\nname.riot"));
        assert!(stderr.is_empty());

        let (status, stdout, stderr) = invoke(&["inspect", artifact.to_str().unwrap()], &clock);
        assert_eq!(status, ExitCode::SUCCESS, "{stderr}");
        assert!(stdout.contains("name: Hello Riot"));
        assert!(stdout.contains("resources:\n  app.js\n  index.html\n"));
        assert!(stderr.is_empty());
    }

    #[test]
    fn argument_validation_names_each_failure_without_touching_files() {
        let clock = || Ok(1_u64);
        let cases = [
            (&["inspect"][..], "inspect requires exactly one bundle file"),
            (
                &["pack", "app", "--key-dir", "keys"][..],
                "pack requires --out <file>",
            ),
            (
                &["pack", "app", "--out", "out"][..],
                "pack requires --key-dir <dir>",
            ),
            (
                &[
                    "pack",
                    "app",
                    "--key-dir",
                    "keys",
                    "--out",
                    "out",
                    "--timestamp-micros",
                    "nope",
                ][..],
                "--timestamp-micros must be an unsigned 64-bit integer",
            ),
            (
                &["pack", "app", "--out", "one", "--out", "two"][..],
                "unexpected or repeated argument '--out'",
            ),
            (
                &["pack", "app", "bad\nflag"][..],
                "missing value for bad\\nflag",
            ),
        ];
        for (args, expected) in cases {
            let (status, stdout, stderr) = invoke(args, &clock);
            assert_eq!(status, ExitCode::FAILURE);
            assert!(stdout.is_empty());
            assert!(stderr.contains(expected), "{stderr:?}");
        }
    }

    #[test]
    fn unix_micros_helpers_cover_success_pre_epoch_and_overflow() {
        assert!(SystemClock.unix_micros().unwrap() > 0);
        assert_eq!(unix_micros_at(UNIX_EPOCH).unwrap(), 0);
        assert_eq!(
            unix_micros_at(UNIX_EPOCH - std::time::Duration::from_micros(1)).unwrap_err(),
            "system clock is before the Unix epoch"
        );
        assert_eq!(
            duration_to_micros(std::time::Duration::from_secs(u64::MAX)).unwrap_err(),
            "current Unix timestamp does not fit in u64 microseconds"
        );
    }

    #[test]
    fn command_failures_cover_filesystem_limits_invalid_artifacts_and_output_errors() {
        let clock = || Ok(1_u64);
        let temp = tempfile::tempdir_in(env!("CARGO_MANIFEST_DIR")).unwrap();
        let missing = temp.path().join("missing.riot");
        let (status, _, stderr) = invoke(&["inspect", missing.to_str().unwrap()], &clock);
        assert_eq!(status, ExitCode::FAILURE);
        assert!(stderr.contains("read input metadata"));

        let oversized = temp.path().join("oversized.riot");
        let oversized_file = std::fs::File::create(&oversized).unwrap();
        oversized_file
            .set_len(riot_core::import::MAX_BUNDLE_BYTES as u64 + 1)
            .unwrap();
        let (status, _, stderr) = invoke(&["inspect", oversized.to_str().unwrap()], &clock);
        assert_eq!(status, ExitCode::FAILURE);
        assert!(stderr.contains("input is too large"));

        let invalid = temp.path().join("invalid.riot");
        std::fs::write(&invalid, b"not a Riot artifact").unwrap();
        let (status, _, stderr) = invoke(&["inspect", invalid.to_str().unwrap()], &clock);
        assert_eq!(status, ExitCode::FAILURE);
        assert!(stderr.contains("strict decoding rejected it"));

        let keys = temp.path().join("keys");
        let mut stdout = FailingWriter;
        let mut stderr = Vec::new();
        assert_eq!(
            main_with(
                vec!["keygen".into(), "--out".into(), keys.as_os_str().to_owned(),],
                &clock,
                &mut stdout,
                &mut stderr,
            ),
            ExitCode::FAILURE
        );
        assert!(String::from_utf8(stderr)
            .unwrap()
            .contains("write output: injected broken pipe"));

        let keys = temp.path().join("warning-keys");
        let mut stdout = Vec::new();
        let mut stderr = FailingWriter;
        assert_eq!(
            main_with(
                vec!["keygen".into(), "--out".into(), keys.as_os_str().to_owned(),],
                &clock,
                &mut stdout,
                &mut stderr,
            ),
            ExitCode::FAILURE
        );
    }

    #[test]
    fn command_adapters_propagate_each_library_and_writer_failure() {
        let clock = || Ok(1_u64);
        let temp = tempfile::tempdir_in(env!("CARGO_MANIFEST_DIR")).unwrap();
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/hello-app");
        let keys = temp.path().join("keys");
        keygen(&keys).unwrap();

        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        assert_eq!(
            main_with(
                vec!["keygen".into(), "--out".into(), keys.as_os_str().into()],
                &clock,
                &mut stdout,
                &mut stderr,
            ),
            ExitCode::FAILURE
        );
        let stderr = String::from_utf8(stderr).unwrap();
        assert!(stderr.contains("overwrite existing"), "{stderr:?}");

        let missing_keys = temp.path().join("missing-keys");
        let missing_output = temp.path().join("missing-output.riot");
        let (status, _, stderr) = invoke(
            &[
                "pack",
                fixture.to_str().unwrap(),
                "--key-dir",
                missing_keys.to_str().unwrap(),
                "--out",
                missing_output.to_str().unwrap(),
                "--timestamp-micros",
                "1",
            ],
            &clock,
        );
        assert_eq!(status, ExitCode::FAILURE);
        assert!(stderr.contains("open key directory"));

        let invalid_app = temp.path().join("missing-app");
        let invalid_output = temp.path().join("invalid-app.riot");
        let (status, _, stderr) = invoke(
            &[
                "pack",
                invalid_app.to_str().unwrap(),
                "--key-dir",
                keys.to_str().unwrap(),
                "--out",
                invalid_output.to_str().unwrap(),
                "--timestamp-micros",
                "1",
            ],
            &clock,
        );
        assert_eq!(status, ExitCode::FAILURE);
        assert!(stderr.contains("open app directory"));

        let existing_output = temp.path().join("existing.riot");
        std::fs::write(&existing_output, b"preserve").unwrap();
        let (status, _, stderr) = invoke(
            &[
                "pack",
                fixture.to_str().unwrap(),
                "--key-dir",
                keys.to_str().unwrap(),
                "--out",
                existing_output.to_str().unwrap(),
                "--timestamp-micros",
                "1",
            ],
            &clock,
        );
        assert_eq!(status, ExitCode::FAILURE);
        assert!(stderr.contains("overwrite existing"), "{stderr:?}");
        assert_eq!(std::fs::read(&existing_output).unwrap(), b"preserve");

        let writer_output = temp.path().join("writer.riot");
        let mut stdout = FailingWriter;
        assert!(run_pack(
            &[
                fixture.to_string_lossy().into_owned(),
                "--key-dir".into(),
                keys.to_string_lossy().into_owned(),
                "--out".into(),
                writer_output.to_string_lossy().into_owned(),
                "--timestamp-micros".into(),
                "1".into(),
            ],
            &clock,
            &mut stdout,
        )
        .unwrap_err()
        .contains("write output"));

        let artifact = temp.path().join("inspect.riot");
        let mut sink = Vec::new();
        run_pack(
            &[
                fixture.to_string_lossy().into_owned(),
                "--key-dir".into(),
                keys.to_string_lossy().into_owned(),
                "--out".into(),
                artifact.to_string_lossy().into_owned(),
                "--timestamp-micros".into(),
                "1".into(),
            ],
            &clock,
            &mut sink,
        )
        .unwrap();
        assert!(run_inspect_with(
            &[artifact.to_string_lossy().into_owned()],
            &mut Vec::new(),
            &ScriptedInput {
                fail_open: true,
                oversized_read: false,
            },
        )
        .unwrap_err()
        .contains("open input: injected open failure"));
        assert!(run_inspect_with(
            &[artifact.to_string_lossy().into_owned()],
            &mut Vec::new(),
            &ScriptedInput {
                fail_open: false,
                oversized_read: true,
            },
        )
        .unwrap_err()
        .contains("input is too large"));
        assert!(run_inspect_with(
            &[artifact.to_string_lossy().into_owned()],
            &mut Vec::new(),
            &ScriptedInput {
                fail_open: false,
                oversized_read: false,
            },
        )
        .is_ok());
        assert!(run_inspect(
            &[artifact.to_string_lossy().into_owned()],
            &mut FailingWriter
        )
        .unwrap_err()
        .contains("write output"));
        assert!(run_inspect(
            &[artifact.to_string_lossy().into_owned()],
            &mut FailAfterLines { lines: 0, limit: 5 }
        )
        .unwrap_err()
        .contains("write output"));

        assert!(run_inspect(
            &[temp.path().to_string_lossy().into_owned()],
            &mut Vec::new()
        )
        .unwrap_err()
        .contains("read input"));
    }

    #[test]
    fn pack_option_predicates_cover_repeated_and_unknown_truth_cases() {
        let clock = || Ok(1_u64);
        let cases = [
            vec![
                "app",
                "--key-dir",
                "one",
                "--key-dir",
                "two",
                "--out",
                "out",
            ],
            vec![
                "app",
                "--key-dir",
                "keys",
                "--out",
                "out",
                "--timestamp-micros",
                "1",
                "--timestamp-micros",
                "2",
            ],
            vec!["app", "--unknown", "value"],
        ];
        for args in cases {
            let mut stdout = Vec::new();
            let error = run_pack(
                &args.into_iter().map(str::to_owned).collect::<Vec<_>>(),
                &clock,
                &mut stdout,
            )
            .unwrap_err();
            assert!(error.contains("unexpected or repeated argument"));
            assert!(stdout.is_empty());
        }
        assert!(run_pack(&[], &clock, &mut Vec::new())
            .unwrap_err()
            .contains("usage:"));
        assert!(ensure_input_size(0, 1).is_ok());
        assert!(ensure_input_size(2, 1)
            .unwrap_err()
            .contains("input is too large"));
    }
}
