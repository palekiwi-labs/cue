mod cli;
mod commands;
mod config;
mod context;
mod git;

use crate::cli::{Cli, Commands};
use anyhow::Context;
use clap::Parser;
use std::env;
use std::io::{self, Cursor, Read};

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let cwd = env::current_dir()?;

    match cli.command {
        Commands::Init => {
            commands::init::handle(&cwd)?;
        }
        Commands::Add {
            filename,
            content,
            file,
            clipboard,
            mem_type,
            force,
            branch,
        } => {
            let resolved_content: Vec<u8> = if clipboard {
                resolve_clipboard(&filename)?
            } else if let Some(path) = file {
                std::fs::read(&path).with_context(|| format!("Failed to read file {}", path))?
            } else {
                let c = content.unwrap_or_else(|| "-".to_string());
                if c == "-" {
                    let mut buf = Vec::new();
                    io::stdin()
                        .read_to_end(&mut buf)
                        .context("Failed to read from stdin")?;
                    buf
                } else {
                    c.into_bytes()
                }
            };

            commands::add::handle(&cwd, &filename, resolved_content, mem_type, force, branch)?;
        }
        Commands::List {
            branch,
            all,
            mem_type,
            include_gitignored,
            json,
        } => {
            commands::list::handle(&cwd, branch, all, mem_type, include_gitignored, json)?;
        }
        Commands::Log { command } => {
            commands::log::handle(&cwd, command)?;
        }
        Commands::Context { command } => {
            commands::context::handle(&cwd, command)?;
        }
    }

    Ok(())
}

fn resolve_clipboard(filename: &str) -> anyhow::Result<Vec<u8>> {
    use arboard::Clipboard;
    use image::{ImageBuffer, ImageFormat, RgbaImage};

    let lower_filename = filename.to_lowercase();
    let is_png = lower_filename.ends_with(".png");
    let is_jpg = lower_filename.ends_with(".jpg") || lower_filename.ends_with(".jpeg");

    // Check for other image formats we don't support yet
    let other_image = [".webp", ".gif", ".bmp", ".tiff", ".tga"];
    if other_image.iter().any(|ext| lower_filename.ends_with(ext)) {
        anyhow::bail!(
            "Unsupported image format in filename '{}'. Supported formats: .png, .jpg, .jpeg",
            filename
        );
    }

    let mut ctx = Clipboard::new().context(
        "Failed to access clipboard. Ensure a display server (X11 or Wayland) is running.",
    )?;

    if is_png || is_jpg {
        let img_data = ctx
            .get_image()
            .context("Clipboard does not contain an image.")?;
        let img: RgbaImage = ImageBuffer::from_raw(
            img_data.width as u32,
            img_data.height as u32,
            img_data.bytes.into_owned(),
        )
        .context("Invalid image data in clipboard")?;

        let mut buf = Vec::new();
        let format = if is_png {
            ImageFormat::Png
        } else {
            ImageFormat::Jpeg
        };
        img.write_to(&mut Cursor::new(&mut buf), format)
            .context("Failed to encode image")?;
        Ok(buf)
    } else {
        // Assume text for any other extension
        let text = ctx.get_text().context("Clipboard does not contain text.")?;
        Ok(text.into_bytes())
    }
}
