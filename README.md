# CFDsTrader

Proyecto entre amigos para crear un algoritmo que **sugiere operaciones con CFDs** combinando análisis técnico, liquidez, noticias en tiempo real y situaciones históricas parecidas, con probabilidades y P/L. Por defecto **todo es simulación**: el sistema propone y un responsable decide.

- Reglas para colaborar (humanos y agentes de IA): [`CLAUDE.md`](CLAUDE.md) · [`CONTRIBUTING.md`](CONTRIBUTING.md)
- Qué hace el algoritmo: [`docs/spec/fase-1-especificacion.md`](docs/spec/fase-1-especificacion.md)
- Arquitectura y módulos: [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) · Fases: [`docs/ROADMAP.md`](docs/ROADMAP.md)
- ¿Un término que no conocés? [`docs/GLOSARIO.md`](docs/GLOSARIO.md)

## Estructura

| Carpeta | Qué hay |
|---|---|
| `engine/` | Motor en **Rust** (workspace de Cargo) |
| `intelligence/` | Inteligencia en **Python** (proyecto `uv`) |
| `ui/` | Prototipo visual de la interfaz (**React + TypeScript + Tailwind**), con datos de ejemplo |
| `infra/` | PostgreSQL + Redis para desarrollo (`docker-compose.yml`) |
| `contracts/` | Contratos JSON Schema entre lenguajes |
| `config/` | Toda la configuración (nada hardcodeado) |
| `docs/` | Especificación, arquitectura, decisiones y glosario |

## Levantar el entorno

Requisitos: Rust (la versión la fija `engine/rust-toolchain.toml`), [uv](https://docs.astral.sh/uv/) y Docker.

```bash
cp .env.example .env    # y completar valores; el .env nunca se commitea
```

```bash
docker compose -f infra/docker-compose.yml --env-file .env up -d
```

## Correr los checks (los mismos que el CI)

Motor (Rust):

```bash
cd engine && cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
```

Inteligencia (Python):

```bash
cd intelligence && uv sync && uv run ruff format --check && uv run ruff check && uv run mypy && uv run pytest
```

Tests de integración con PostgreSQL y Redis (los mismos que corre el CI; con el `docker compose` de arriba levantado):

```bash
cd engine && DATABASE_URL=postgres://<usuario>:<contraseña>@127.0.0.1:5432/<base> REDIS_URL=redis://127.0.0.1:6379 cargo test --workspace -- --ignored
```

Interfaz (prototipo, TypeScript): `npm run dev` y abrir la dirección que muestra.

```bash
cd ui && npm ci && npm run lint && npm run format:check && npm run typecheck && npm test && npm run build
```
