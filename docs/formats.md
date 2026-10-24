# Formatos, perfiles y metadatos

## Matriz de la versión 0.1.0

| Entrada | Salida | Sin pérdida | Notas |
|---|---|---|---|
| JPEG 8 bits | JPEG | No | Calidad configurable; alfa se aplana |
| PNG 8/16 bits | PNG | Sí | 16 bits se reduce a 8; oxipng |
| WebP estático | WebP | Sí/No | libwebp; animados se omiten |
| AVIF | — | — | Fuera de 0.1.0 |

La detección usa magic bytes, no la extensión. Un archivo con extensión engañosa se procesa según su contenido real y se avisa.

## Perfiles

| Perfil | Formato | Calidad | Pérdida | Redimensionado |
|---|---|---|---|---|
| Compartir | Auto | 82 | JPEG/WebP | No |
| Ahorrar espacio | Auto | 90 JPEG | PNG/WebP sin pérdida | No |
| Web | WebP | 80 | Sí | No |
| Personalizado | Elegible | 85 | Elegible | Opcional |

El perfil Compartir no convierte a WebP por defecto porque la conversión puede aumentar el tamaño de imágenes pequeñas o con alfa. El perfil Web sí convierte a WebP y conserva alfa.

## Color

- La orientación EXIF de JPEG se aplica antes de codificar; la salida no lleva una orientación que pueda duplicar la rotación.
- ICC se conserva en JPEG (APP2) y PNG (iCCP) cuando la política es `ColorOnly`.
- WebP no incrusta ICC en 0.1.0; se avisa en el análisis.
- No se realiza conversión de espacio de color a sRGB en esta versión. Si el perfil no se puede conservar, se avisa y se acepta el riesgo de cambio de color.
- CMYK: el decodificador puede rechazarlo; se reporta como error, no se inventa una conversión.

## Metadatos

En 0.1.0 todos los perfiles eliminan EXIF, GPS y XMP. `ColorOnly` conserva ICC; `StripAll` también lo elimina. No hay una opción de «conservar todo» todavía; se documentará cuando exista.

## Tamaño y calidad

- No se modifica la orientación, la transparencia ni las dimensiones salvo que el usuario active el redimensionado.
- El redimensionado solo reduce el lado mayor y nunca amplía.
- No se garantiza que el resultado sea menor. La política por defecto omite el archivo si crece.
- No se publican cifras de ahorro universales: dependen de la imagen, el formato y la calidad.

## Diferencias respecto al plan inicial

- Se descartó mozjpeg para la v1 y se usa el codificador JPEG puro Rust de `image`. Motivo: evitar CMake/NASM y reducir el riesgo de empaquetado. La calidad es suficiente para la v1, pero es un área de mejora medida.
- AVIF queda para una versión posterior.
- La conservación de ICC en WebP requiere un contenedor extendido y no se implementó.
