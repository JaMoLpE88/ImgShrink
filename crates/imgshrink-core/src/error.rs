use std::path::PathBuf;

/// Errores del núcleo. Cada variante tiene un código estable para la UI y el informe.
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("No se pudo leer \u{00AB}{path}\u{00BB}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("No se pudo escribir \u{00AB}{path}\u{00BB}: {source}")]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("Formato no admitido: {0}")]
    Unsupported(String),
    #[error("Imagen dañada o no decodificable: {0}")]
    Decode(String),
    #[error("Error al codificar la imagen: {0}")]
    Encode(String),
    #[error("La imagen supera los límites de memoria o dimensiones permitidos")]
    TooLarge,
    #[error("El resultado no es menor que el original")]
    NotSmaller,
    #[error("Ya existe un archivo en \u{00AB}{0}\u{00BB}")]
    Exists(PathBuf),
    #[error("El archivo original cambió durante el procesamiento")]
    SourceChanged,
    #[error("Operación cancelada")]
    Cancelled,
    #[error("Configuración inválida: {0}")]
    InvalidConfig(String),
    #[error("{0}")]
    Other(String),
}

impl CoreError {
    pub fn code(&self) -> &'static str {
        match self {
            CoreError::Read { .. } => "IS-IO-001",
            CoreError::Write { .. } => "IS-IO-002",
            CoreError::Unsupported(_) => "IS-IMG-001",
            CoreError::Decode(_) => "IS-IMG-002",
            CoreError::Encode(_) => "IS-IMG-003",
            CoreError::TooLarge => "IS-IMG-004",
            CoreError::NotSmaller => "IS-IMG-010",
            CoreError::Exists(_) => "IS-IO-010",
            CoreError::SourceChanged => "IS-IO-011",
            CoreError::Cancelled => "IS-OP-001",
            CoreError::InvalidConfig(_) => "IS-CFG-001",
            CoreError::Other(_) => "IS-GEN-001",
        }
    }
}

impl From<std::io::Error> for CoreError {
    fn from(value: std::io::Error) -> Self {
        CoreError::Other(value.to_string())
    }
}

impl From<image::ImageError> for CoreError {
    fn from(value: image::ImageError) -> Self {
        CoreError::Decode(value.to_string())
    }
}
