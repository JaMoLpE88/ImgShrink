use std::path::{Path, PathBuf};
use std::time::Duration;

pub fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    if bytes < 1024 {
        return format!("{} B", bytes);
    }
    let mut value = bytes as f64;
    let mut unit = 0usize;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    format!("{:.1} {}", value, UNITS[unit]).replace('.', ",")
}

pub fn format_duration(duration: Duration) -> String {
    let secs = duration.as_secs();
    if secs < 60 {
        format!("{}.{} s", secs, duration.subsec_tenths())
    } else if secs < 3600 {
        format!("{} min {} s", secs / 60, secs % 60)
    } else {
        format!("{} h {} min", secs / 3600, (secs % 3600) / 60)
    }
}

trait SubsecTenths {
    fn subsec_tenths(&self) -> u32;
}

impl SubsecTenths for Duration {
    fn subsec_tenths(&self) -> u32 {
        self.subsec_millis() / 100
    }
}

/// Clave de comparación para detectar colisiones de nombres en sistemas
/// insensibles a mayúsculas. En Linux se mantiene el nombre original.
pub fn normalize_key(path: &Path) -> String {
    use unicode_normalization::UnicodeNormalization;
    let normalized: String = path.to_string_lossy().nfc().collect();
    let text = normalized.replace('\\', "/");
    if cfg!(any(windows, target_os = "macos")) {
        text.to_lowercase()
    } else {
        text
    }
}

pub fn validate_suffix(raw: &str) -> Result<String, String> {
    let suffix = raw.trim().to_string();
    if suffix.is_empty() {
        return Err("El sufijo no puede estar vacío".into());
    }
    if suffix.len() > 40 {
        return Err("El sufijo es demasiado largo (máximo 40 caracteres)".into());
    }
    if suffix
        .chars()
        .any(|c| matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'))
    {
        return Err("El sufijo no puede contener separadores ni caracteres reservados".into());
    }
    if suffix.starts_with('.') || suffix.ends_with('.') || suffix.ends_with(' ') {
        return Err("El sufijo no puede empezar o terminar con punto o espacio".into());
    }
    if is_windows_reserved(&suffix) {
        return Err("El sufijo coincide con un nombre reservado de Windows".into());
    }
    Ok(suffix)
}

pub fn is_windows_reserved(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    let base = upper.split('.').next().unwrap_or(&upper);
    matches!(base, "CON" | "PRN" | "AUX" | "NUL")
        || (base.len() == 4
            && (base.starts_with("COM") || base.starts_with("LPT"))
            && base.as_bytes()[3].is_ascii_digit())
}

/// Devuelve una ruta libre añadiendo -1, -2, ... antes de la extensión.
pub fn unique_path(path: &Path) -> PathBuf {
    if !path.exists() {
        return path.to_path_buf();
    }
    let parent = path.parent().unwrap_or_else(|| Path::new(""));
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "imagen".into());
    let ext = path.extension().map(|s| s.to_string_lossy().to_string());
    for index in 1..10_000 {
        let name = match &ext {
            Some(ext) => format!("{}-{}.{}", stem, index, ext),
            None => format!("{}-{}", stem, index),
        };
        let candidate = parent.join(name);
        if !candidate.exists() {
            return candidate;
        }
    }
    path.to_path_buf()
}

pub fn display_path(path: &Path) -> String {
    let text = path.to_string_lossy().to_string();
    #[cfg(windows)]
    {
        text.strip_prefix("\\\\?\\").unwrap_or(&text).to_string()
    }
    #[cfg(not(windows))]
    {
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suffix_validation() {
        assert!(validate_suffix("-imgshrink").is_ok());
        assert!(validate_suffix("").is_err());
        assert!(validate_suffix("a/b").is_err());
        assert!(validate_suffix(".hidden").is_err());
        assert!(validate_suffix("CON").is_err());
    }

    #[test]
    fn nfc_and_nfd_names_collide() {
        use std::path::Path;
        let composed = Path::new("canci\u{00F3}n.png");
        let decomposed = Path::new("cancio\u{0301}n.png");
        assert_eq!(normalize_key(composed), normalize_key(decomposed));
    }

    #[test]
    fn bytes_are_human_readable() {
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1536), "1,5 KB");
    }
}
