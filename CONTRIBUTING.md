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
