# Estrategia de pruebas

## Capas

| Capa | Qué cubre | Dónde |
|---|---|---|
| Unitaria | Formatos, rutas, planes, estadísticas, códecs, ICC, escritura segura | `crates/imgshrink-core/src/**` |
| Integración por lotes | Carpeta separada, sufijo, in-place, conflictos, renombrado, dañados | `crates/imgshrink-core/tests/batch.rs` |
| Renderizado TUI | Pantallas a 120×40, 80×24, 60×20, 50×14 y 40×12 | `crates/imgshrink-tui/tests/render.rs` |
| Flujo TUI | Selección → análisis → destino → revisión → proceso → resumen | `crates/imgshrink-tui/tests/flow.rs` |
| PTY | Arranque real del binario y restauración al salir | `crates/imgshrink-cli/tests/pty.rs` |
| Empaquetado | `--selftest` en limpio | CI y verificación manual |

## Comandos

```bash
cargo test --workspace
cargo test -p imgshrink-core --test batch
cargo test -p imgshrink-tui --test render
cargo test -p imgshrink-cli --test pty
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
cargo run -q -p imgshrink-cli -- --selftest
```

## Fixtures

Las imágenes de prueba se generan en tiempo de ejecución con `image` y `libwebp`, no se guardan binarios opacos en el repositorio. Los casos corruptos se construyen truncando o alterando los magic bytes. Esto mantiene el repositorio auditable y evita licencias de imágenes de terceros.

## PTY y ConPTY

En Windows, ConPTY emite una consulta de posición del cursor (`ESC[6n`) al iniciar. La prueba responde con `ESC[1;1R` antes de enviar teclas. En una terminal real responde el emulador. Si la prueba se ejecuta en un entorno que no responde a CPR, se queda esperando: es una limitación del harness, no de la aplicación.

## Criterios de aceptación

1. `cargo test --workspace` pasa en la plataforma de desarrollo.
2. `cargo clippy -- -D warnings` no produce avisos.
3. `imgshrink --selftest` procesa JPEG, PNG y WebP y verifica las salidas.
4. Un fallo de validación no modifica el original.
5. La TUI no entra en pánico a 50×14 ni a 40×12.
6. La terminal se restaura tras una salida normal.
7. El informe refleja solo éxitos en el ahorro.
8. Las plataformas no verificadas se marcan como tales en README y packaging.

## Pendiente para fases posteriores

- Pruebas de cancelación deterministas con un motor instrumentado.
- Pruebas de permisos y archivos bloqueados en Windows.
- Fuzz de cabeceras y metadatos.
- Benchmarks de rendimiento y memoria.
- Verificación en macOS y Linux reales.
- Pruebas de firma/notarización.
