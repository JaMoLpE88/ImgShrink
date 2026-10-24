use serde::{Deserialize, Serialize};

use crate::format::ImageFormat;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Purpose {
    Share,
    SaveSpace,
    Web,
    Custom,
}

impl Purpose {
    pub const ALL: [Purpose; 4] = [
        Purpose::Share,
        Purpose::SaveSpace,
        Purpose::Web,
        Purpose::Custom,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Purpose::Share => "Compartir",
            Purpose::SaveSpace => "Ahorrar espacio",
            Purpose::Web => "Web",
            Purpose::Custom => "Personalizado",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Purpose::Share => "Equilibrio entre peso y calidad para mensajería, correo y redes.",
            Purpose::SaveSpace => {
                "Conserva la mayor información posible; sin pérdida cuando el formato lo permite."
            }
            Purpose::Web => "Convierte a WebP con buena calidad para publicación web.",
            Purpose::Custom => "Elige formato, calidad, tamaño y metadatos manualmente.",
        }
    }

    pub fn tag(self) -> &'static str {
        match self {
            Purpose::Share => "Recomendado para enviar",
            Purpose::SaveSpace => "Máxima conservación",
            Purpose::Web => "WebP / publicación",
            Purpose::Custom => "Control total",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutputFormat {
    Auto,
    Jpeg,
    Png,
    WebP,
}

impl OutputFormat {
    pub const ALL: [OutputFormat; 4] = [
        OutputFormat::Auto,
        OutputFormat::Jpeg,
        OutputFormat::Png,
        OutputFormat::WebP,
    ];

    pub fn label(self) -> &'static str {
        match self {
            OutputFormat::Auto => "Mantener formato",
            OutputFormat::Jpeg => "JPEG",
            OutputFormat::Png => "PNG",
            OutputFormat::WebP => "WebP",
        }
    }

    pub fn resolve(self, source: ImageFormat) -> ImageFormat {
        match self {
            OutputFormat::Auto => source,
            OutputFormat::Jpeg => ImageFormat::Jpeg,
            OutputFormat::Png => ImageFormat::Png,
            OutputFormat::WebP => ImageFormat::WebP,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MetadataPolicy {
    /// Conserva el perfil ICC cuando el formato lo permite; elimina EXIF, GPS y XMP.
    ColorOnly,
    /// Elimina todos los metadatos, incluido el perfil ICC.
    StripAll,
}

impl MetadataPolicy {
    pub fn label(self) -> &'static str {
        match self {
            MetadataPolicy::ColorOnly => "Conservar color (ICC); eliminar EXIF/GPS",
            MetadataPolicy::StripAll => "Eliminar todos los metadatos",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OptimizeOptions {
    pub output_format: OutputFormat,
    pub quality: u8,
    pub lossless: bool,
    pub max_dimension: Option<u32>,
    pub metadata: MetadataPolicy,
    pub keep_if_larger: bool,
    pub background: [u8; 3],
    pub png_effort: u8,
}

impl Default for OptimizeOptions {
    fn default() -> Self {
        Self::for_purpose(Purpose::Share)
    }
}

impl OptimizeOptions {
    pub fn for_purpose(purpose: Purpose) -> Self {
        match purpose {
            Purpose::Share => Self {
                output_format: OutputFormat::Auto,
                quality: 82,
                lossless: false,
                max_dimension: None,
                metadata: MetadataPolicy::ColorOnly,
                keep_if_larger: true,
                background: [255, 255, 255],
                png_effort: 3,
            },
            Purpose::SaveSpace => Self {
                output_format: OutputFormat::Auto,
                quality: 90,
                lossless: true,
                max_dimension: None,
                metadata: MetadataPolicy::ColorOnly,
                keep_if_larger: true,
                background: [255, 255, 255],
                png_effort: 4,
            },
            Purpose::Web => Self {
                output_format: OutputFormat::WebP,
                quality: 80,
                lossless: false,
                max_dimension: None,
                metadata: MetadataPolicy::ColorOnly,
                keep_if_larger: true,
                background: [255, 255, 255],
                png_effort: 3,
            },
            Purpose::Custom => Self {
                output_format: OutputFormat::Auto,
                quality: 85,
                lossless: false,
                max_dimension: None,
                metadata: MetadataPolicy::ColorOnly,
                keep_if_larger: true,
                background: [255, 255, 255],
                png_effort: 3,
            },
        }
    }

    pub fn effective_format(&self, source: ImageFormat) -> ImageFormat {
        self.output_format.resolve(source)
    }

    /// Ajusta el perfil cuando el usuario cambia el formato de salida a JPEG:
    /// no tiene sentido mantener la marca de sin pérdida.
    pub fn normalize_for(&mut self, source: ImageFormat) {
        let fmt = self.effective_format(source);
        if fmt == ImageFormat::Jpeg {
            self.lossless = false;
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.quality > 100 {
            return Err("La calidad debe estar entre 0 y 100".into());
        }
        if let Some(max) = self.max_dimension {
            if max < 16 {
                return Err("El tamaño máximo debe ser al menos 16 px".into());
            }
        }
        if self.png_effort > 6 {
            return Err("El esfuerzo PNG debe estar entre 0 y 6".into());
        }
        Ok(())
    }
}
