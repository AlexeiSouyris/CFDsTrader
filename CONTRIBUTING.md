# Cómo contribuir

Las reglas completas están en [CLAUDE.md](./CLAUDE.md) y valen igual para humanos y agentes. Este es el resumen del flujo:

1. **Issue primero.** Buscá si ya existe un Issue o PR para lo mismo. Si no, crealo con la plantilla.
2. **Reclamalo**: asignátelo y agregá la etiqueta `in-progress`. Si ya está tomado, no lo trabajes.
3. **Rama**: `<tipo>/<nº-issue>-<descripcion>` desde `main` actualizado.
4. **Contratos primero** si tu cambio cruza Rust, Python y UI (`contracts/`).
5. **Programá + tests.** Si creás o cambiás un módulo, actualizá su ficha en `docs/modules/` y regenerá el *Registro de módulos* de `docs/ARCHITECTURE.md` con `python3 tools/docs_index.py`.
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

## Herramientas para agentes: graphify (opcional)

[graphify](https://github.com/Graphify-Labs/graphify) arma un grafo de conocimiento del repo para que los agentes de IA lo consulten en lugar de leer archivos enteros, y así gasten menos tokens. **Es opcional y local:** el repo no lo trae configurado y los agentes trabajan normal sin él.

Si lo querés usar en tu máquina, instalá el CLI con la versión fijada:

```bash
uv tool install "graphifyy==0.9.113"
```

Registrá el skill y los hooks de Claude Code en tu copia del repo:

```bash
graphify install --project
```

Generá el grafo (local, sin LLM y sin costo):

```bash
graphify update .
```

**No commitees lo que genera `graphify install --project`.** Ese comando:
- crea `.claude/settings.json`, `.claude/CLAUDE.md` y `.claude/skills/graphify/`;
- agrega una sección `## graphify` (en inglés) al final de `CLAUDE.md`.

Para que esos archivos no aparezcan en `git status`, podés ignorarlos solo en tu máquina con `.git/info/exclude`, que no se commitea:

```bash
printf '.claude/settings.json\n.claude/CLAUDE.md\n.claude/skills/graphify/\n' >> .git/info/exclude
```

El cambio en `CLAUDE.md` no se puede ignorar así porque es un archivo del repo. Antes de commitear, revisá `git diff CLAUDE.md` y dejá afuera la sección `## graphify`.

- `graphify-out/` ya está en `.gitignore`.
- `/graphify .` dentro del asistente además hace una pasada **semántica** sobre los documentos usando el modelo, y eso **sí consume tokens**. Para el día a día alcanza con `graphify update .`.
- Si tenés `GEMINI_API_KEY` o `GOOGLE_API_KEY` en tu entorno, esa pasada semántica manda los documentos a Gemini.
- La versión está fijada. Subirla es un PR aparte.
