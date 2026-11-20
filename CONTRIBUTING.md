# Cómo contribuir

Las reglas completas están en [CLAUDE.md](./CLAUDE.md) y valen igual para humanos y agentes. Este es el resumen del flujo:

1. **Issue primero.** Buscá si ya existe un Issue o PR para lo mismo. Si no, crealo con la plantilla.
2. **Reclamalo**: asignátelo y agregá la etiqueta `in-progress`. Si ya está tomado, no lo trabajes.
3. **Rama**: `<tipo>/<nº-issue>-<descripcion>` desde `main` actualizado.
4. **Contratos primero** si tu cambio cruza Rust, Python y UI (`contracts/`).
5. **Programá + tests.** Actualizá el *Registro de módulos* en `docs/ARCHITECTURE.md` si creás o cambiás un módulo.
6. `git rebase origin/main`, pasá los checks y abrí el PR con la plantilla (`Closes #nº`).
7. Cualquiera puede **revisar y comentar**. El **merge a `main` lo aprueba y lo hace el dueño del repo (Alexei)**, con *squash*. Nadie más mergea por su cuenta.

## Etiquetas

| Etiqueta | Uso |
|---|---|
| `in-progress` | Alguien (o su agente) está trabajando en esto. **No tocar.** |
| `engine` / `intelligence` / `infra` / `ui` / `contracts` | Área del cambio |
| `feat` / `bug` / `refactor` / `docs` | Tipo |
| `needs-decision` | Requiere un ADR o acuerdo de los dos |
| `blocked` | Espera otro Issue o PR (indicá cuál) |

## Configuración recomendada en GitHub (la hace el dueño del repo)

- *Settings → Branches*: proteger `main` (exigir PR y aprobación antes del merge; prohibir *force push*). Dejar el merge en manos del dueño.
- *Settings → General*: permitir solo **Squash merge** y borrar las ramas automáticamente al mergear.

## Herramientas para agentes: graphify

[graphify](https://github.com/Graphify-Labs/graphify) arma un grafo de conocimiento del repo para que los agentes de IA lo consulten en lugar de leer archivos enteros, y así gasten menos tokens. El skill y los hooks de Claude Code ya están en `.claude/`. Cada persona instala el CLI una vez:

```bash
uv tool install "graphifyy==0.9.113"
```

y genera el grafo (local, sin LLM y sin costo):

```bash
graphify update .
```

- `graphify-out/` no se commitea: cada uno lo genera en su máquina.
- Si no instalás el CLI, los hooks de `.claude/settings.json` fallan sin bloquear nada y el agente trabaja normal.
- Ojo: `/graphify .` dentro del asistente además hace una pasada **semántica** sobre los documentos usando el modelo, y eso **sí consume tokens**. Para el día a día alcanza con `graphify update .`.
- La versión está fijada. Subirla es un PR aparte.
