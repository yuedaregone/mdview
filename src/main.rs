#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

mod app;
mod cli;
mod config;
mod context_menu;
mod file_watcher;
mod font;
mod markdown;
mod selection;
mod theme;
mod update;
mod viewport;
mod windows_console;

use cli::{Args, CliCommand};

fn main() -> eframe::Result<()> {
    let raw_args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
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
            println!("{}", cli::usage_text());
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

    // Use command line file if provided
    let file_to_open = args.file.clone();

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

#[cfg(all(target_os = "windows", not(debug_assertions)))]
fn prepare_cli_console(raw_args: &[std::ffi::OsString], parsed_args: Option<&Args>) -> bool {
    if !cli::should_prepare_cli_console(raw_args, parsed_args) {
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
fn prepare_cli_console(raw_args: &[std::ffi::OsString], parsed_args: Option<&Args>) -> bool {
    cli::should_prepare_cli_console(raw_args, parsed_args)
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
