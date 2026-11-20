# Interfaz de CFDsTrader: prototipo

**Maqueta con datos de ejemplo** (Issue #30). No se conecta al motor ni a IG, y **no calcula nada**: la UI solo muestra lo que envía el motor (CLAUDE.md §1). La app real es la fase 10.

## Agregar un componente (por ejemplo de 21st.dev)

1. Copiar el componente en `src/components/` (si usa `cn` de `@/lib/utils`, ya está).
2. Si necesita datos, ponerlos en `src/mocks/`, marcados como ejemplo.
3. Sumarlo a la lista de `src/gallery.ts`, con título, para qué lo usaríamos y el link al original.

## Comandos

```bash
npm ci          # instalar
npm run dev     # ver la página en el navegador
npm test        # tests
```
