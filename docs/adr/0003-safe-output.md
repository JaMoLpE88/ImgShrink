# ADR 0003: salida segura

Estado: aceptado.

## Decisión

Toda salida se escribe en un temporal de la misma carpeta, se sincroniza, se valida decodificándola y solo después se mueve al destino. En in-place se comprueba que el original no cambió y se registra un journal antes del reemplazo.

## Motivos

- Evitar que un fallo de codificación destruya el original.
- Detectar resultados corruptos antes de publicarlos.
- Detectar operaciones inacabadas tras un cierre abrupto.

## Límites

- No hay atomicidad universal en unidades de red.
- El journal detecta restos; no recupera automáticamente.
- Un bloqueo de antivirus puede hacer fallar el reemplazo; el original permanece intacto.
