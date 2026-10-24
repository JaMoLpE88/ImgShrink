# ADR 0002: motor de imagen sin libvips

Estado: aceptado para la v1.

## Contexto

libvips es rápido y consume poca memoria, pero arrastra GLib y varias bibliotecas nativas; enlazarlo estáticamente complica el binario único y añade obligaciones LGPL.

## Decisión

Motor principalmente Rust:

- `image` para decodificar.
- Codificador JPEG de `image` (puro Rust).
- `png` + `oxipng` para PNG.
- `webp`/libwebp para WebP con y sin pérdida.
- `kamadak-exif` para orientación.

## Consecuencias

- Empaquetado mucho más simple.
- Calidad JPEG inferior a mozjpeg; se puede medir y sustituir en el futuro.
- Sin procesamiento en streaming de libvips; se mitiga con límites de memoria y trabajadores acotados.
- AVIF queda fuera hasta validar decodificador y empaquetado.
