# Arquitectura de ImgShrink

## Principios

1. La interfaz no contiene lógica de compresión.
2. El núcleo se prueba sin terminal.
3. Los originales no se tocan salvo confirmación explícita.
4. El progreso refleja lo que el códec puede medir; no se inventan porcentajes.
5. Las dependencias nativas se compilan dentro del binario.

## Crates y dirección de dependencias

```text
imgshrink-cli
   ├── imgshrink-tui
   │      └── imgshrink-core
   └── imgshrink-core
```

- `imgshrink-core`: dominio puro. No depende de ratatui ni de crossterm.
- `imgshrink-tui`: estado de la aplicación, pantallas, tema, entrada y ciclo de terminal.
- `imgshrink-cli`: argumentos, logging, `--selftest`, `--diagnose` y arranque de la TUI.

## Módulos del núcleo

| Módulo | Responsabilidad |
|---|---|
| `format` | Detección por magic bytes, extensiones y capacidades |
| `probe` | Dimensiones, color, alfa, animación, orientación, ICC y errores de cabecera |
| `discovery` | Recorrido de archivos y carpetas, recursividad, enlaces, deduplicación |
| `options` | Perfiles, formato de salida, calidad, metadatos y validación |
| `plan` | Rutas de salida, conflictos, omisiones previas y avisos |
| `engine` | Decodificación, orientación, redimensionado, codificación y validación |
| `output` | Temporal, escritura, validación, reemplazo y política de conflictos |
| `journal` | Registro de reemplazos in-place para detectar operaciones inacabadas |
| `jobs` | Pool de trabajadores, eventos, cancelación y agregación |
| `stats` | Ahorro calculado solo con resultados correctos |
| `report` | Informe JSON local |
| `config` | Configuración TOML y rutas de estado |

## Flujo de una operación

```text
TUI
 │  selección + opciones + destino
 ▼
discover() ──► Vec<SourceItem>
 │
 ▼
build_plan() ──► OperationPlan (rutas, conflictos, omisiones, avisos)
 │
 ▼  revisión y confirmación
jobs::run()
 │  pool de hilos + canales acotados
 ├── engine::optimize_with()  [Decode → Transform → Encode]
 ├── comparación de tamaño / política
 ├── output::write_item()     [Verify → Write → Replace]
 └── eventos Totals / ItemFinished
 │
 ▼
Summary + informe JSON + pantalla final
```

## Estado de la interfaz

```text
Welcome → Purpose → Source → Analysis → Destination → Review → Running → Summary
```

Estados auxiliares: ayuda, ajustes, confirmación de sobrescritura, lista de errores y edición de texto. La UI es inmediata (ratatui); el estado vive en `App` y se reduce a partir de eventos.

## Concurrencia

- Hilos del sistema, sin runtime async.
- Cola acotada de índices de trabajo; el plan no guarda píxeles.
- Canal acotado para eventos de etapa: si se llena, se descarta la etapa intermedia.
- Canal separado para resultados finales; no se descartan.
- `Arc<AtomicBool>` para cancelación; se comprueba antes de cada trabajo y antes de escribir.
- La UI drena eventos y redibuja como máximo ~20 FPS; nunca bloquea a un trabajador.

## Seguridad de archivos

1. Se calcula la ruta de salida en el plan.
2. Se detectan colisiones y archivos existentes antes de procesar.
3. Se escribe en un temporal de la misma carpeta.
4. Se hace `flush` y `sync_all`.
5. Se vuelve a decodificar el temporal.
6. En in-place se comprueba que el original no cambió (tamaño y mtime).
7. Se registra el journal y se reemplaza con la primitiva del sistema.
8. Se elimina la entrada del journal.

Limitaciones: la atomicidad no está garantizada en unidades de red, sistemas exóticos o con bloqueos externos. El journal permite detectar restos, no recuperarlos automáticamente.

## Puntos de extensión previstos

- AVIF: nuevo valor en `ImageFormat`, codificador `ravif` y decodificador validado.
- Backend libvips opcional: detrás de una feature, sin cambiar la UI.
- Informe CSV: extensión de `report`.
- i18n: extraer cadenas de la TUI a un catálogo.
