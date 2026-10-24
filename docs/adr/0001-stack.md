# ADR 0001: Rust + ratatui

Estado: aceptado.

## Contexto

Se necesita una TUI compleja, binarios portables y procesamiento de imágenes local sin runtimes externos.

## Decisión

Rust estable, ratatui/crossterm y un núcleo independiente de la UI.

## Alternativas

- Go + Bubble Tea: excelente TUI y binarios, pero el motor WebP con calidad exige CGO/libvips.
- Python + Textual: gran productividad, pero PyInstaller/Nuitka no cumplen bien el requisito portable.
- Node + Ink: ecosistema y `sharp`, pero empaquetar módulos nativos en un ejecutable es frágil.
- C++ + FTXUI: rendimiento, pero mayor coste de seguridad y empaquetado.

## Consecuencias

- Más código de UI que con un framework de componentes.
- Un solo binario y seguridad de memoria en el procesamiento.
- La compilación de libwebp requiere un compilador de C.
