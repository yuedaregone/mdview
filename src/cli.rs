use std::ffi::OsString;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliCommand {
    Run,
    Register,
    Unregister,
    Help,
    Version,
}

impl CliCommand {
    pub fn flag_name(self) -> &'static str {
        match self {
            CliCommand::Run => "run mode",
            CliCommand::Register => "--register",
            CliCommand::Unregister => "--unregister",
            CliCommand::Help => "--help",
            CliCommand::Version => "--version",
        }
    }
}

#[derive(Debug)]
pub struct Args {
    pub file: Option<PathBuf>,
    pub command: CliCommand,
}

impl Args {
    pub fn parse_from<I>(args: I) -> Result<Self, String>
    where
        I: IntoIterator<Item = OsString>,
    {
        let mut file = None;
        let mut command = CliCommand::Run;
        let mut positional_only = false;

        for arg in args {
            if !positional_only {
                if arg == "--" {
                    positional_only = true;
                    continue;
                }
                if let Some(option) = arg.to_str() {
                    match option {
                        "--register" => {
                            command = Self::set_command(command, CliCommand::Register, option)?;
                            continue;
                        }
                        "--unregister" => {
                            command = Self::set_command(command, CliCommand::Unregister, option)?;
                            continue;
                        }
                        "--help" | "-h" => {
                            command = Self::set_command(command, CliCommand::Help, option)?;
                            continue;
                        }
                        "--version" | "-V" => {
                            command = Self::set_command(command, CliCommand::Version, option)?;
                            continue;
                        }
                        _ => {}
                    }
                    if option.starts_with('-') {
                        return Err(format!("Unknown option: {option}"));
                    }
                }
            }

            if file.is_none() {
                file = Some(PathBuf::from(arg));
            } else {
                return Err("Only one input file is supported".to_string());
            }
        }

        if command != CliCommand::Run && file.is_some() {
            return Err("File arguments cannot be combined with command flags".to_string());
        }

        Ok(Self { file, command })
    }

    fn set_command(
        current: CliCommand,
        next: CliCommand,
        option: &str,
    ) -> Result<CliCommand, String> {
        if current == CliCommand::Run || current == next {
            Ok(next)
        } else {
            Err(format!(
                "Cannot combine {option} with {}",
                current.flag_name()
            ))
        }
    }
}

pub fn usage_text() -> String {
    format!(
        "{name} {version}
{description}

USAGE:
  {name} [OPTIONS] [FILE]
  {name} --register
  {name} --unregister

ARGS:
  [FILE]          Markdown file to open

OPTIONS:
  -h, --help      Show this help message
  -V, --version   Show version information
      --register  Register Windows file association for .md files
      --unregister
                  Remove Windows file association for .md files

NOTES:
  Use `--` before a path that starts with `-`, for example:
  {name} -- --demo.md",
        name = env!("CARGO_PKG_NAME"),
        version = env!("CARGO_PKG_VERSION"),
        description = env!("CARGO_PKG_DESCRIPTION"),
    )
}

pub fn should_prepare_cli_console(raw_args: &[OsString], parsed_args: Option<&Args>) -> bool {
    if let Some(args) = parsed_args {
        matches!(
            args.command,
            CliCommand::Register | CliCommand::Unregister | CliCommand::Help | CliCommand::Version
        )
    } else {
        !raw_args.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::{Args, CliCommand, should_prepare_cli_console, usage_text};
    use std::ffi::OsString;
    use std::path::PathBuf;

    fn parse(args: &[&str]) -> Result<Args, String> {
        Args::parse_from(args.iter().map(OsString::from))
    }

    #[test]
    fn parses_single_file_argument() {
        let args = parse(&["README.md"]).unwrap();
        assert_eq!(args.file, Some(PathBuf::from("README.md")));
        assert_eq!(args.command, CliCommand::Run);
    }

    #[test]
    fn parses_register_flag() {
        let args = parse(&["--register"]).unwrap();
        assert_eq!(args.command, CliCommand::Register);
        assert_eq!(args.file, None);
    }

    #[test]
    fn supports_double_dash_for_dash_prefixed_paths() {
        let args = parse(&["--", "--demo.md"]).unwrap();
        assert_eq!(args.file, Some(PathBuf::from("--demo.md")));
        assert_eq!(args.command, CliCommand::Run);
    }

    #[test]
    fn parses_help_flag() {
        let args = parse(&["--help"]).unwrap();
        assert_eq!(args.command, CliCommand::Help);
        assert_eq!(args.file, None);
    }

    #[test]
    fn parses_version_flag() {
        let args = parse(&["-V"]).unwrap();
        assert_eq!(args.command, CliCommand::Version);
    }

    #[test]
    fn rejects_unknown_flags() {
        let err = parse(&["--bogus"]).unwrap_err();
        assert!(err.contains("Unknown option"));
    }

    #[test]
    fn rejects_multiple_files() {
        let err = parse(&["a.md", "b.md"]).unwrap_err();
        assert!(err.contains("Only one input file"));
    }

    #[test]
    fn rejects_conflicting_command_flags() {
        let err = parse(&["--register", "--unregister"]).unwrap_err();
        assert!(err.contains("--unregister"));
    }

    #[test]
    fn rejects_file_with_command_flag() {
        let err = parse(&["--help", "README.md"]).unwrap_err();
        assert!(err.contains("File arguments"));
    }

    #[test]
    fn usage_mentions_double_dash_escape() {
        let help = usage_text();
        assert!(help.contains("`--`"));
        assert!(help.contains("-- --demo.md"));
    }

    #[test]
    fn gui_run_mode_does_not_request_console() {
        let args = parse(&["README.md"]).unwrap();
        let raw_args = vec![OsString::from("README.md")];
        assert!(!should_prepare_cli_console(&raw_args, Some(&args)));
    }

    #[test]
    fn help_mode_requests_console() {
        let args = parse(&["--help"]).unwrap();
        let raw_args = vec![OsString::from("--help")];
        assert!(should_prepare_cli_console(&raw_args, Some(&args)));
    }

    #[test]
    fn parse_errors_with_args_request_console() {
        let raw_args = vec![OsString::from("a.md"), OsString::from("b.md")];
        assert!(should_prepare_cli_console(&raw_args, None));
    }
}
