# Fichas de módulos

Una ficha por módulo: `docs/modules/<módulo>.md`. La tabla *Registro de módulos* de [`docs/ARCHITECTURE.md`](../ARCHITECTURE.md#registro-de-módulos) se **genera** desde el front-matter de estas fichas (#669), así dos PRs que tocan módulos distintos ya no chocan en una misma línea.

## Formato

```markdown
---
modulo: ejemplo
ruta: `engine/crates/ejemplo` (crate `cfds-ejemplo`)
lenguaje: Rust
estado: en curso (#123)
resumen: Una sola oración corta (hasta 160 caracteres) con la responsabilidad del módulo.
---

# ejemplo

## Responsabilidad (única)

- Qué hace y qué no, en detalle (una viñeta por tema).

## Estado

- Qué está hecho y en qué Issue o PR.
```

- `modulo` es igual al nombre del archivo (sin `.md`).
- `estado`: `planeado` → `en curso (#issue)` → `hecho`.
- Una clave por línea, `clave: valor`, sin comillas.

## Cómo agregar o cambiar un módulo

1. Creá o editá `docs/modules/<módulo>.md`.
2. Regenerá la tabla: `python3 tools/docs_index.py`.
3. Commiteá la ficha y `docs/ARCHITECTURE.md` juntos. El CI corre `python3 tools/docs_index.py --check` y falla si la tabla no está al día.

El detalle va en la ficha. La tabla solo cambia cuando cambian la ruta, el lenguaje, el estado o el resumen.
