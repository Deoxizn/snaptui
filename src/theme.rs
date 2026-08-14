use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::{anyhow, Context, Result};
use ratatui::style::Color;
use serde::Deserialize;

#[derive(Clone, Debug)]
pub struct Theme {
    pub bg: Color,
    pub fg: Color,
    pub accent: Color,
    pub muted: Color,
    pub surface: Color,
    pub error: Color,
    pub warn: Color,
    pub ok: Color,
    pub cyan: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            bg: Color::Rgb(0x14, 0x11, 0x1a),
            fg: Color::Rgb(0xd8, 0xd0, 0xdc),
            accent: Color::Rgb(0xb6, 0x84, 0x9d),
            muted: Color::Rgb(0x6b, 0x5e, 0x72),
            surface: Color::Rgb(0x2a, 0x24, 0x32),
            error: Color::Rgb(0xb5, 0x7a, 0x92),
            warn: Color::Rgb(0x9a, 0x84, 0xa2),
            ok: Color::Rgb(0x6f, 0x7b, 0x8f),
            cyan: Color::Rgb(0x7e, 0x95, 0xc1),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ThemeSource {
    pub path: PathBuf,
    pub mtime: SystemTime,
}

#[derive(Deserialize)]
struct ColorsToml {
    accent: Option<String>,
    muted: Option<String>,
    background: Option<String>,
    lighter_background: Option<String>,
    foreground: Option<String>,
    red: Option<String>,
    yellow: Option<String>,
    green: Option<String>,
    cyan: Option<String>,
}

fn parse_color(s: &str) -> Option<Color> {
    let s = s.trim();
    let s = s.strip_prefix('#')?;
    if s.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&s[0..2], 16).ok()?;
    let g = u8::from_str_radix(&s[2..4], 16).ok()?;
    let b = u8::from_str_radix(&s[4..6], 16).ok()?;
    Some(Color::Rgb(r, g, b))
}

fn active_theme_slug() -> Option<String> {
    let out = std::process::Command::new("omarchy")
        .args(["theme", "current"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let name = String::from_utf8_lossy(&out.stdout);
    let slug = name.trim().to_lowercase();
    if slug.is_empty() {
        None
    } else {
        Some(slug)
    }
}

fn candidate_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(home) = std::env::var_os("HOME") {
        let home = PathBuf::from(home);
        paths.push(home.join(".config/omarchy/themes"));
        if let Some(slug) = active_theme_slug() {
            paths.push(home.join(format!(".config/omarchy/themes/{slug}/colors.toml")));
            paths.push(PathBuf::from(format!("/usr/share/omarchy/themes/{slug}/colors.toml")));
        }
    }
    paths
}

pub fn load_theme_from(path: &Path) -> Result<(Theme, ThemeSource)> {
    let data = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let mtime = fs::metadata(path)
        .and_then(|m| m.modified())
        .with_context(|| format!("metadata for {}", path.display()))?;
    let colors: ColorsToml =
        toml::from_str(&data).map_err(|e| anyhow!("parsing {}: {e}", path.display()))?;

    let c = |opt: &Option<String>, fallback: Color| -> Color {
        opt.as_deref().and_then(parse_color).unwrap_or(fallback)
    };

    let theme = Theme {
        bg: c(&colors.background, Theme::default().bg),
        fg: c(&colors.foreground, Theme::default().fg),
        accent: c(&colors.accent, Theme::default().accent),
        muted: c(&colors.muted, Theme::default().muted),
        surface: c(&colors.lighter_background, Theme::default().surface),
        error: c(&colors.red, Theme::default().error),
        warn: c(&colors.yellow, Theme::default().warn),
        ok: c(&colors.green, Theme::default().ok),
        cyan: c(&colors.cyan, Theme::default().cyan),
    };
    Ok((theme, ThemeSource { path: path.to_path_buf(), mtime }))
}

pub fn load_theme_from_any() -> Result<Option<(Theme, ThemeSource)>> {
    for p in candidate_paths() {
        if p.file_name().map(|f| f.to_string_lossy().ends_with("colors.toml")) == Some(true) {
            if let Ok(loaded) = load_theme_from(&p) {
                return Ok(Some(loaded));
            }
        }
    }
    Ok(None)
}

pub fn reload_if_changed(_theme: &Theme, source: &Option<ThemeSource>) -> Result<Option<(Theme, ThemeSource)>> {
    let Some(source) = source else { return Ok(None) };
    let mtime = match fs::metadata(&source.path).and_then(|m| m.modified()) {
        Ok(m) => m,
        Err(_) => return Ok(None),
    };
    if mtime != source.mtime {
        load_theme_from(&source.path).map(Some).map_err(Into::into)
    } else {
        Ok(None)
    }
}
