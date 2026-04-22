use clap::Parser;
use std::path::PathBuf;

mod app;
mod config;
mod image_loader;
mod markdown;
mod selection;
mod theme;
mod viewport;

/// A blazingly fast, ultra-lightweight Markdown reader
#[derive(Parser, Debug)]
#[command(name = "mdview", version, about)]
struct Args {
    /// Markdown file to open
    file: Option<PathBuf>,

    /// Register .md file association (Windows only)
    #[arg(long)]
    register: bool,

    /// Unregister .md file association (Windows only)
    #[arg(long)]
    unregister: bool,
}

fn main() -> eframe::Result<()> {
    let args = Args::parse();

    // Handle file association registration
    if args.register {
        register_file_association();
        return Ok(());
    }
    if args.unregister {
        unregister_file_association();
        return Ok(());
    }

    // Load config for window settings
    let config = config::AppConfig::load();

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

    let mut viewport_builder = egui::ViewportBuilder::default()
        .with_inner_size([config.window_width, config.window_height])
        .with_min_inner_size([400.0, 300.0])
        .with_title("mdview");

    if let (Some(x), Some(y)) = (config.window_x, config.window_y) {
        viewport_builder = viewport_builder.with_position(egui::Pos2::new(x, y));
    }

    if config.maximized {
        viewport_builder = viewport_builder.with_maximized(true);
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
            let app = app::MdViewApp::new(cc, doc, file_to_open.clone());
            Ok(Box::new(app))
        }),
    )
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
        .args(["-ExecutionPolicy", "Bypass", "-NoProfile", "-Command", &ps_script])
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
        .args(["-ExecutionPolicy", "Bypass", "-NoProfile", "-Command", ps_script])
        .creation_flags(CREATE_NO_WINDOW)
        .output();

    println!("File association unregistered.");
}

#[cfg(not(target_os = "windows"))]
fn unregister_file_association() {
    eprintln!("File association is only supported on Windows.");
}
