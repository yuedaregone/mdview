use clap::Parser;
use std::path::PathBuf;

mod app;
mod image_loader;
mod markdown;
mod selection;
mod theme;
mod viewport;
mod widgets;

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

    // Pre-parse markdown before window init for faster perceived startup
    let doc = args
        .file
        .as_ref()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|content| markdown::parser::parse_full(&content));

    let mut native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([900.0, 700.0])
            .with_min_inner_size([400.0, 300.0])
            .with_title("mdview"),
        ..Default::default()
    };

    // Set window title to filename if provided
    if let Some(ref path) = args.file {
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
            let app = app::MdViewApp::new(cc, doc, args.file.clone());
            Ok(Box::new(app))
        }),
    )
}

#[cfg(target_os = "windows")]
fn register_file_association() {
    let exe = std::env::current_exe().unwrap_or_default();
    let exe_str = exe.to_string_lossy();

    // Write to HKCU (no admin needed)
    let commands = [
        format!(
            "reg add HKCU\\Software\\Classes\\.md /ve /d mdview.md /f"
        ),
        format!(
            "reg add HKCU\\Software\\Classes\\mdview.md /ve /d \"Markdown File\" /f"
        ),
        format!(
            "reg add HKCU\\Software\\Classes\\mdview.md\\shell\\open\\command /ve /d \"\\\"{}\\\" \\\"%%1\\\"\" /f",
            exe_str
        ),
        format!(
            "reg add HKCU\\Software\\Classes\\mdview.md\\DefaultIcon /ve /d \"\\\"{}\\\",0\" /f",
            exe_str
        ),
    ];

    for cmd in &commands {
        match std::process::Command::new("cmd").args(["/C", cmd]).output() {
            Ok(_) => {}
            Err(e) => eprintln!("Failed to register: {}", e),
        }
    }
    println!("File association registered. .md files will now open with mdview.");
}

#[cfg(not(target_os = "windows"))]
fn register_file_association() {
    eprintln!("File association is only supported on Windows.");
}

#[cfg(target_os = "windows")]
fn unregister_file_association() {
    let commands = [
        "reg delete HKCU\\Software\\Classes\\.md /ve /f".to_string(),
        "reg delete HKCU\\Software\\Classes\\mdview.md /f".to_string(),
    ];
    for cmd in &commands {
        let _ = std::process::Command::new("cmd").args(["/C", cmd]).output();
    }
    println!("File association unregistered.");
}

#[cfg(not(target_os = "windows"))]
fn unregister_file_association() {
    eprintln!("File association is only supported on Windows.");
}
