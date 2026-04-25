#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

use std::ffi::OsString;
use std::path::PathBuf;

mod app;
mod config;
mod context_menu;
mod file_watcher;
mod font;
mod markdown;
mod selection;
mod theme;
mod update;
mod viewport;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CliCommand {
    Run,
    Register,
    Unregister,
    Help,
    Version,
}

impl CliCommand {
    fn flag_name(self) -> &'static str {
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
struct Args {
    file: Option<PathBuf>,
    command: CliCommand,
}

impl Args {
    fn parse_from<I>(args: I) -> Result<Self, String>
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

fn main() -> eframe::Result<()> {
    let raw_args: Vec<OsString> = std::env::args_os().skip(1).collect();
    let parsed_args = Args::parse_from(raw_args.clone());
    let console_ready = prepare_cli_console(&raw_args, parsed_args.as_ref().ok());
    let args = match parsed_args {
        Ok(args) => args,
        Err(err) => exit_with_cli_error(&err, console_ready),
    };

    match args.command {
        CliCommand::Register => {
            register_file_association();
            return Ok(());
        }
        CliCommand::Unregister => {
            unregister_file_association();
            return Ok(());
        }
        CliCommand::Help => {
            println!("{}", usage_text());
            return Ok(());
        }
        CliCommand::Version => {
            println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        CliCommand::Run => {}
    }

    // Load config for window settings
    let mut config = config::AppConfig::load();

    // Use command line file if provided, otherwise use last opened file
    let file_to_open = args
        .file
        .clone()
        .or_else(|| config.last_file.as_ref().map(PathBuf::from));

    // Pre-parse markdown before window init for faster perceived startup
    let doc = file_to_open
        .as_ref()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|content| markdown::parser::parse_full(&content));

    let prepared_fonts = font::prepare_fonts(&mut config);
    let theme = resolve_startup_theme(&config);
    let file_watcher = file_watcher::SimpleFileWatcher::new(file_to_open.clone());

    let mut viewport_builder = egui::ViewportBuilder::default()
        .with_min_inner_size([400.0, 300.0])
        .with_title("mdview")
        .with_decorations(false)
        .with_transparent(true);

    if config.maximized {
        viewport_builder = viewport_builder.with_maximized(true);
    } else {
        viewport_builder = viewport_builder.with_inner_size([config.window_width, config.window_height]);
        if let (Some(x), Some(y)) = (config.window_x, config.window_y) {
            viewport_builder = viewport_builder.with_position(egui::Pos2::new(x, y));
        }
    }

    let mut native_options = eframe::NativeOptions {
        viewport: viewport_builder,
        persist_window: false,
        ..Default::default()
    };

    // Set window title to filename if provided
    if let Some(ref path) = file_to_open {
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            native_options.viewport = native_options
                .viewport
                .with_title(format!("{} — mdview", name));
        }
    }

    eframe::run_native(
        "mdview",
        native_options,
        Box::new(move |cc| {
            let bootstrap = app::AppBootstrap {
                config: config.clone(),
                doc,
                file_path: file_to_open.clone(),
                theme,
                file_watcher,
                prepared_fonts,
            };
            let app = app::MdViewApp::new(cc, bootstrap);
            Ok(Box::new(app))
        }),
    )
}

fn resolve_startup_theme(config: &config::AppConfig) -> theme::Theme {
    let themes = theme::Theme::from_config();
    if let Some(theme_name) = config.theme_name.as_deref() {
        themes
            .into_iter()
            .find(|theme| theme.name == theme_name)
            .unwrap_or_else(theme::Theme::default_theme)
    } else {
        theme::Theme::default_theme()
    }
}

fn should_prepare_cli_console(raw_args: &[OsString], parsed_args: Option<&Args>) -> bool {
    if let Some(args) = parsed_args {
        matches!(
            args.command,
            CliCommand::Register | CliCommand::Unregister | CliCommand::Help | CliCommand::Version
        )
    } else {
        !raw_args.is_empty()
    }
}

#[cfg(all(target_os = "windows", not(debug_assertions)))]
fn prepare_cli_console(raw_args: &[OsString], parsed_args: Option<&Args>) -> bool {
    if !should_prepare_cli_console(raw_args, parsed_args) {
        return false;
    }

    if windows_console::has_standard_output_handles() {
        return true;
    }

    if !windows_console::attach_parent_console() {
        return false;
    }

    windows_console::bind_standard_streams();
    true
}

#[cfg(not(all(target_os = "windows", not(debug_assertions))))]
fn prepare_cli_console(raw_args: &[OsString], parsed_args: Option<&Args>) -> bool {
    should_prepare_cli_console(raw_args, parsed_args)
}

fn exit_with_cli_error(message: &str, console_ready: bool) -> ! {
    #[cfg(not(all(target_os = "windows", not(debug_assertions))))]
    let _ = console_ready;
    eprintln!("{message}");
    #[cfg(all(target_os = "windows", not(debug_assertions)))]
    if !console_ready {
        let _ = rfd::MessageDialog::new()
            .set_title("mdview")
            .set_description(message)
            .set_level(rfd::MessageLevel::Error)
            .show();
    }
    std::process::exit(2);
}

#[cfg(target_os = "windows")]
fn register_file_association() {
    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Failed to get executable path: {}", e);
            return;
        }
    };
    let exe_str = exe.to_string_lossy();

    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;

    let escaped_exe = exe_str.replace("'", "''");

    let ps_script = format!(
        r#"
        $ErrorActionPreference = 'Stop'
        try {{
            $exe = '{}'
            New-Item -Path 'HKCU:\Software\Classes\.md' -Force | Out-Null
            Set-ItemProperty -Path 'HKCU:\Software\Classes\.md' -Name '(Default)' -Value 'mdview.md'
            New-Item -Path 'HKCU:\Software\Classes\mdview.md' -Force | Out-Null
            Set-ItemProperty -Path 'HKCU:\Software\Classes\mdview.md' -Name '(Default)' -Value 'Markdown File'
            New-Item -Path 'HKCU:\Software\Classes\mdview.md\shell\open\command' -Force | Out-Null
            Set-ItemProperty -Path 'HKCU:\Software\Classes\mdview.md\shell\open\command' -Name '(Default)' -Value "`"$exe`" `"%1`""
            New-Item -Path 'HKCU:\Software\Classes\mdview.md\DefaultIcon' -Force | Out-Null
            Set-ItemProperty -Path 'HKCU:\Software\Classes\mdview.md\DefaultIcon' -Name '(Default)' -Value "`"$exe`",0"
            Write-Host 'File association registered successfully.'
        }} catch {{
            Write-Error "Failed to register: $_"
            exit 1
        }}
        "#,
        escaped_exe
    );

    match std::process::Command::new("powershell")
        .args([
            "-ExecutionPolicy",
            "Bypass",
            "-NoProfile",
            "-Command",
            &ps_script,
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
    {
        Ok(output) => {
            if output.status.success() {
                println!("File association registered. .md files will now open with mdview.");
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                eprintln!("Failed to register: {}", stderr);
            }
        }
        Err(e) => eprintln!("Failed to execute PowerShell: {}", e),
    }
}

#[cfg(not(target_os = "windows"))]
fn register_file_association() {
    eprintln!("File association is only supported on Windows.");
}

#[cfg(target_os = "windows")]
fn unregister_file_association() {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;

    let ps_script = r#"
        $ErrorActionPreference = 'SilentlyContinue'
        Remove-Item -Path 'HKCU:\Software\Classes\.md' -Recurse -Force
        Remove-Item -Path 'HKCU:\Software\Classes\mdview.md' -Recurse -Force
        Write-Host 'File association unregistered.'
    "#;

    let _ = std::process::Command::new("powershell")
        .args([
            "-ExecutionPolicy",
            "Bypass",
            "-NoProfile",
            "-Command",
            ps_script,
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output();

    println!("File association unregistered.");
}

#[cfg(not(target_os = "windows"))]
fn unregister_file_association() {
    eprintln!("File association is only supported on Windows.");
}

fn usage_text() -> String {
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

#[cfg(all(target_os = "windows", not(debug_assertions)))]
mod windows_console {
    use std::ffi::c_void;
    use std::fs::OpenOptions;
    use std::os::windows::io::IntoRawHandle;

    const ATTACH_PARENT_PROCESS: u32 = u32::MAX;
    const STD_INPUT_HANDLE: u32 = -10i32 as u32;
    const STD_OUTPUT_HANDLE: u32 = -11i32 as u32;
    const STD_ERROR_HANDLE: u32 = -12i32 as u32;

    #[link(name = "kernel32")]
    extern "system" {
        fn AttachConsole(dw_process_id: u32) -> i32;
        fn GetStdHandle(n_std_handle: u32) -> *mut c_void;
        fn SetStdHandle(n_std_handle: u32, handle: *mut c_void) -> i32;
    }

    pub fn attach_parent_console() -> bool {
        unsafe { AttachConsole(ATTACH_PARENT_PROCESS) != 0 }
    }

    pub fn has_standard_output_handles() -> bool {
        let stdout = unsafe { GetStdHandle(STD_OUTPUT_HANDLE) };
        let stderr = unsafe { GetStdHandle(STD_ERROR_HANDLE) };
        is_valid_handle(stdout) && is_valid_handle(stderr)
    }

    pub fn bind_standard_streams() {
        bind_input_stream();
        bind_output_stream("CONOUT$", STD_OUTPUT_HANDLE);
        bind_output_stream("CONOUT$", STD_ERROR_HANDLE);
    }

    fn bind_input_stream() {
        if let Ok(file) = OpenOptions::new().read(true).open("CONIN$") {
            let handle = file.into_raw_handle();
            unsafe {
                let _ = SetStdHandle(STD_INPUT_HANDLE, handle as *mut c_void);
            }
        }
    }

    fn bind_output_stream(device: &str, std_handle: u32) {
        if let Ok(file) = OpenOptions::new().write(true).open(device) {
            let handle = file.into_raw_handle();
            unsafe {
                let _ = SetStdHandle(std_handle, handle as *mut c_void);
            }
        }
    }

    fn is_valid_handle(handle: *mut c_void) -> bool {
        !handle.is_null() && handle as isize != -1
    }
}

#[cfg(test)]
mod tests {
    use super::{usage_text, Args, CliCommand};
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
        assert!(!super::should_prepare_cli_console(&raw_args, Some(&args)));
    }

    #[test]
    fn help_mode_requests_console() {
        let args = parse(&["--help"]).unwrap();
        let raw_args = vec![OsString::from("--help")];
        assert!(super::should_prepare_cli_console(&raw_args, Some(&args)));
    }

    #[test]
    fn parse_errors_with_args_request_console() {
        let raw_args = vec![OsString::from("a.md"), OsString::from("b.md")];
        assert!(super::should_prepare_cli_console(&raw_args, None));
    }
}
