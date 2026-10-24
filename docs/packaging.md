# Empaquetado y distribución

## Objetivo

Un usuario descarga un archivo, lo descomprime, ejecuta `imgshrink` y no instala runtimes ni bibliotecas.

## Estrategia por plataforma

| Plataforma | Target | Enlace | Artefacto |
|---|---|---|---|
| Windows 11 x64 | `x86_64-pc-windows-msvc` | CRT estático | ZIP |
| Windows 11 ARM64 | `aarch64-pc-windows-msvc` | CRT estático | ZIP (teórico) |
| macOS Apple Silicon | `aarch64-apple-darwin` | Bibliotecas del sistema | tar.gz |
| macOS Intel | `x86_64-apple-darwin` | Bibliotecas del sistema | tar.gz |
| Linux x64 musl | `x86_64-unknown-linux-musl` | Estático | tar.gz |
| Linux x64 glibc | `x86_64-unknown-linux-gnu` | glibc del sistema | tar.gz |
| Linux ARM64 | `aarch64-unknown-linux-musl` | Estático | tar.gz (teórico) |

## Dependencias nativas

- **libwebp**: se compila desde fuente mediante `libwebp-sys` y `cc`.
- **libdeflate**: se compila desde fuente.
- No se usa FFmpeg, ImageMagick, Python, Node.js ni libvips.
- En Windows se necesita MSVC Build Tools para compilar. No se necesita NASM ni CMake para las dependencias actuales.

## Windows

```powershell
$env:RUSTFLAGS = "-C target-feature=+crt-static"
cargo build --release --target x86_64-pc-windows-msvc -p imgshrink-cli
.\target\x86_64-pc-windows-msvc\release\imgshrink.exe --selftest
dumpbin /dependents .\target\x86_64-pc-windows-msvc\release\imgshrink.exe
```

Solo deben aparecer DLLs del sistema (kernel32, user32, etc.). Si aparece `vcruntime140.dll`, el CRT no se enlazó estáticamente.

## Linux

```bash
rustup target add x86_64-unknown-linux-musl
cargo build --release --target x86_64-unknown-linux-musl -p imgshrink-cli
./target/x86_64-unknown-linux-musl/release/imgshrink --selftest
ldd ./target/x86_64-unknown-linux-musl/release/imgshrink
```

`ldd` debe responder «not a dynamic executable» o mostrar solo dependencias mínimas del sistema.

## macOS

```bash
rustup target add aarch64-apple-darwin x86_64-apple-darwin
cargo build --release --target aarch64-apple-darwin -p imgshrink-cli
cargo build --release --target x86_64-apple-darwin -p imgshrink-cli
lipo -create -output imgshrink \
  target/aarch64-apple-darwin/release/imgshrink \
  target/x86_64-apple-darwin/release/imgshrink
otool -L imgshrink
```

Gatekeeper puede bloquear binarios sin firma. La firma y notarización requieren una cuenta de desarrollador de Apple y no forman parte de esta entrega.

## Verificación en limpio

1. Copiar el artefacto a una VM/contenedor sin toolchain.
2. `imgshrink --version`.
3. `imgshrink --selftest`.
4. `imgshrink --diagnose`.
5. Abrir la TUI y procesar una carpeta de prueba.
6. Comprobar que no hay bibliotecas no sistémicas.

## Estado real

- Windows 11 x64: compilado, tests y `--selftest` ejecutados el 2026-10-24.
- macOS y Linux: workflows preparados, sin ejecución verificada en esta entrega.
- ARM: sin verificación.

No se debe afirmar que una plataforma está soportada solo por compilar.
