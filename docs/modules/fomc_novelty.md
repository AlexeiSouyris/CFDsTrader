---
modulo: fomc_novelty
ruta: `intelligence/cfds_intel/fomc_novelty`
lenguaje: Python
estado: en curso (#668)
resumen: Novedad de cada comunicado del FOMC respecto del anterior (diff de palabras y embeddings locales), descriptiva.
---

# fomc_novelty

## Responsabilidad (única)

- Mide cuánto cambió el texto de cada comunicado del FOMC respecto del inmediatamente anterior, 2011–2025 (#668). Solo **describe**: no le pide opinión a ningún modelo, no usa precios ni prueba retornos.
- Los comunicados salen de `official_releases` (ADR 0016, solo lectura); el texto, de su página oficial en federalreserve.gov, con las reglas de acceso del ADR 0016 §1.2 (`PoliteFetcher`: un pedido por vez, pausa, sin reintentos) y una copia local en `[client] html_dir` (fuera de git) para no repetir pedidos. Se mide el cuerpo hasta el párrafo de la votación (`[statements] end_marker`); si la página no tiene esa forma, falla.
- Métodos: `word_diff` (1 - 2·M/(n₁+n₂) con difflib sobre palabras) y `embedding_cosine` (distancia coseno de embeddings de Ollama local, por defecto `nomic-embed-text`, fijado por nombre y digest). Config en `config/fomc_novelty.toml`.
- Guarda en `fomc_statement_novelty` (`fomc_statement_novelty.v1`, migración 0050), de solo agregar: una fila por comunicado, método, versión y modelo; cada valor existe desde la hora de su comunicado (`available_at`). Comando `python -m cfds_intel.fomc_novelty [--method ...] [--dry-run] [--csv ...]`.

## Estado

- Diff de palabras y embeddings (#668). La serie descriptiva está en `docs/analysis/2026-11-fomc-novedad.md`.
