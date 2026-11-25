# Reglas del proyecto CFDsTrader (para agentes de IA y humanos)

Este archivo lo leen automáticamente Claude Code y otros agentes (`AGENTS.md` apunta acá).
Las reglas están escritas para **dos personas, cada una con su propio agente de IA**, trabajando en el mismo repo (ver el modo de un solo constructor, abajo).
El objetivo de estas reglas es que **nunca existan dos versiones de lo mismo** y que los PRs sean ordenados.

> Si una regla te impide hacer lo que te pidieron, **no la rompas en silencio**: decíselo a tu humano y proponé el cambio a la regla en un PR aparte.

> **Modo de un solo constructor (vigente desde el 22-11-2026, #155).** Por ahora el proyecto lo construyen **solo el dueño (Alexei) y sus agentes de IA**. Mientras dure, donde estas reglas o los demás documentos piden la aprobación de "las dos personas" o "ambos" para una **fase, un ADR o un cambio de spec**, **alcanza con la aprobación del dueño**:
> - Tiene que ser explícita y quedar registrada en el Issue o en el PR. Si la informa un agente, cita las palabras del dueño y la fecha.
> - Un agente **nunca** se aprueba a sí mismo, ni da por aprobado nada sin esa aprobación explícita.
> - **Excepciones que no cambian:** el **dinero real** (§5 y fase 11) y cualquier paso del modo simulación/paper a órdenes reales siguen pidiendo un ADR aprobado por **dos personas**. Mientras el dueño construya solo, quedan bloqueados.
> - El modo termina cuando se sume otra persona al proyecto. Desde ahí vuelve a regir "las dos personas" para lo que todavía no se aprobó.

---

> **Arranque:** el primer PR, que crea estas reglas, no tiene Issue porque todavía no había ninguno. A partir de su merge, todo pasa por Issues.

## 0. Antes de escribir una sola línea de código (OBLIGATORIO)

1. **Leé** `docs/ARCHITECTURE.md` (mapa de módulos: qué existe, dónde vive y en qué lenguaje).
2. **Buscá si ya existe**: `git fetch origin` y luego
   - `git grep -i "<concepto>" origin/main` (por ejemplo `double_top`, `rsi` o `liquidity`)
   - revisá los **Issues abiertos** y los **PRs abiertos** (incluidos los borradores) por el mismo tema.
3. **Tiene que haber un Issue** para la tarea. Si no existe, crealo con la plantilla.
4. **Reclamá el Issue antes de empezar**: asignátelo y poné la etiqueta `in-progress`.
   - Si el Issue **ya está asignado a otra persona o tiene `in-progress`, NO lo trabajes**. Elegí otro o preguntá en el Issue.
5. Si tu tarea necesita un módulo que **otro está construyendo** (PR abierto), **no lo reimplementes**: esperá, construí sobre su rama o coordiná en el Issue.
6. **Dónde trabajar y cupo de IG:** leé `docs/guides/trabajo-en-paralelo.md`. Las cargas de datos que se quieren conservar van solo en el Mac mini. En la nube, solo pruebas chicas contra IG.

## 1. Arquitectura (fuente de verdad: `docs/ARCHITECTURE.md`)

Monorepo híbrido. Decisión registrada en `docs/decisions/0001-arquitectura-hibrida.md`.

| Carpeta | Lenguaje | Responsabilidad |
|---|---|---|
| `engine/` | **Rust** | Motor principal: datos de mercado, indicadores, patrones de velas, liquidez, estrategias, riesgo/apalancamiento, backtesting, ejecución simulada (paper), API/WebSocket para la UI |
| `intelligence/` | **Python / PyTorch** | Noticias (ingesta + interpretación/sentimiento), búsqueda de situaciones históricas análogas, modelos de ML y predicciones |
| `infra/` | SQL / Docker | PostgreSQL (histórico) + Redis (tiempo real, caché, eventos), migraciones, docker-compose |
| `contracts/` | JSON Schema | **Contratos** de todo mensaje o tabla que cruza entre lenguajes |
| `ui/` | **TypeScript** | Interfaz (fase 2, después del motor) |
| `docs/` | Markdown | Arquitectura, decisiones (ADRs) y guías |

**Regla de oro anti-duplicados: cada cálculo vive en UN solo lugar.**
- Indicadores, patrones, riesgo y P/L → **solo en Rust**. Python **no** recalcula un RSI ni un doble techo: los consume de Redis o PostgreSQL.
- ML, NLP de noticias y análogos históricos → **solo en Python**. Rust consume sus resultados como eventos.
- La UI **nunca** calcula lógica de trading: solo muestra lo que envía el motor.
- Si necesitás algo que vive en otro lenguaje, se comunica por **Redis Streams o PostgreSQL**, usando un contrato de `contracts/`.

**Contratos primero:** cualquier mensaje nuevo entre Rust, Python y la UI se define primero en `contracts/` (JSON Schema versionado), en el mismo PR o en uno previo. Está prohibido inventar campos sin definirlos ahí.

## 2. Ramas, commits y Pull Requests

- **Una rama por Issue**, nombre: `<tipo>/<nº-issue>-<descripcion-corta>`, por ejemplo `feat/12-patrones-velas` o `fix/30-calculo-margen`.
  - Si tu entorno te asigna una rama fija (las sesiones cloud de Claude Code), usala, pero **referenciá el Issue** en el título y el cuerpo del PR.
- Tipos: `feat`, `fix`, `refactor`, `docs`, `test`, `chore`, `perf`.
- **Commits** con Conventional Commits y alcance = módulo: `feat(engine/patterns): detectar doble techo`.
- **Un PR = un Issue = un tema.** Nada de "de paso arreglé otras cosas": eso va en otro Issue.
- PRs **chicos** (idealmente menos de 400 líneas cambiadas sin contar tests ni datos). Si crece, partilo.
- Antes de abrir el PR: `git fetch origin && git rebase origin/main`, y que pasen los checks del punto 4.
- Usá la plantilla de PR (`.github/pull_request_template.md`) y completá **todas** las secciones.
- El cuerpo debe incluir `Closes #<nº>`.
- **El merge a `main` lo aprueba y lo hace siempre el dueño del repo (Alexei).** Ni los agentes ni los colaboradores mergean por su cuenta: abren el PR, y queda esperando la aprobación del dueño. Merge con **squash**.
  - **Delegación vigente (#32, 20-11-2026):** el dueño delegó el merge en **su propio agente** (Claude Code de Alexei), que mergea con squash cuando el **CI está en verde**, no hay conflictos y el PR tiene la plantilla completa. Los PRs de otra persona los revisa primero y, si ve un problema, avisa al dueño en vez de mergear. **Nunca** mergea solo lo que habilite dinero real o toque credenciales o seguridad, y **no da por aprobado ningún ADR**: un ADR se mergea como *propuesta* y aprobarlo sigue siendo de las dos personas. La delegación no alcanza a ningún otro agente, y el dueño la puede revocar en cualquier momento.
- Cualquiera (persona o agente) puede **revisar y comentar** un PR para mejorarlo, pero la decisión de mergear es solo del dueño.
- **Nunca** `git push --force` a `main` ni a la rama de otra persona.
- Al mergear se borra la rama. Si el PR se abandona, quitá `in-progress` y desasignate del Issue.

## 3. Registro de módulos (evita duplicados)

`docs/ARCHITECTURE.md` tiene una tabla **Registro de módulos**, generada desde una ficha por módulo en `docs/modules/<módulo>.md` (formato en `docs/modules/README.md`). Reglas:
- Si tu PR **crea** un módulo nuevo, agregá su ficha en el mismo PR (nombre, ruta, responsabilidad, estado) y regenerá la tabla con `python3 tools/docs_index.py`.
- Si tu PR **cambia la responsabilidad** de un módulo, actualizá su ficha (y regenerá la tabla).
- Antes de crear un archivo o módulo nuevo, verificá que su responsabilidad no esté ya en la tabla.

## 4. Calidad (todo PR debe pasar)

- **Rust** (`engine/`): `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`.
- **Python** (`intelligence/`): `ruff format --check`, `ruff check`, `mypy`, `pytest`. Dependencias con `uv`.
- **TypeScript** (`ui/`): `eslint`, `prettier --check`, `tsc --noEmit`, tests.
- Toda lógica de trading nueva lleva **tests**, incluidos casos borde (gaps, datos faltantes, apalancamiento máximo, margin call).
- Los cálculos financieros usan **decimales exactos** (`rust_decimal` en Rust, `Decimal` en Python) para precios, P/L y margen. Nunca `float` para dinero.
- Todo timestamp va en **UTC** e incluye zona horaria.

## 5. Seguridad y dinero (NO negociable)

- **Modo por defecto: simulación/paper.** No se envían órdenes reales. Cambiar esto requiere un ADR aprobado por las dos personas.
- **Nunca** commitear claves, tokens ni contraseñas. Usá `.env` (está en `.gitignore`) y documentá las variables en `.env.example` con valores falsos.
- Toda estrategia debe pasar por el módulo de **riesgo** (límite de apalancamiento por instrumento, stop-loss obligatorio, pérdida máxima diaria). No hay atajos que lo salteen.
- Las señales muestran **probabilidades e incertidumbre**, nunca certezas. Ningún texto de la UI promete ganancias.

## 5b. Reglas de trading, datos y backtest (anti-autoengaño)

Detalle y criterios de aprobación por fase en `docs/ROADMAP.md`. Resumen obligatorio:

- **Nada de mirar el futuro (look-ahead):** la decisión en la vela `t` usa solo información disponible al **cierre** de `t` y se ejecuta en `t+1`.
- **Solo velas cerradas.** Ningún indicador ni patrón usa la vela en curso. En multi-timeframe, la vela del timeframe mayor se usa recién cuando **cerró**.
- **Sin repintado:** un valor calculado para una vela cerrada no cambia nunca al llegar datos nuevos (hay test que lo verifica).
- **Pivotes confirmados:** un máximo o mínimo local solo existe después de N velas de confirmación. La señal va en la vela de confirmación.
- **Calentamiento:** un indicador no emite señal hasta tener datos suficientes.
- **Definiciones objetivas:** cada patrón tiene una definición medible y parametrizada ("tocado ≥3 veces con tolerancia X%"), nunca "se ve como".
- **Costos nunca en cero:** spread, comisión, slippage y **financiación overnight (swap) del CFD**, todos configurables.
- **Noticias con hora de publicación real:** una noticia solo influye desde el momento en que estuvo disponible, no desde la hora del evento.
- **Datos fuera de muestra intocables:** el set de prueba final no se usa para ajustar nada. Validación walk-forward, nunca un solo período.
- **Nada hardcodeado:** símbolos, tamaños, umbrales, horarios y apalancamientos van en configuración (`config/`), validada al arrancar.
- **Funciones puras** para señales y estrategias: misma entrada, misma salida, sin estado oculto.
- **Órdenes idempotentes** (ID propio), y un **kill switch** ante límite superado, error de datos o desconexión.
- **Registro de resultados:** toda prueba (backtest, walk-forward, ronda de investigación, ML) agrega una fila a `docs/analysis/registro-de-resultados.csv` en el mismo PR. Las filas viejas no se editan: si algo cambia, va una fila nueva que dice contra qué fila compara y si dio mejor, peor o igual (ver `docs/analysis/registro-de-resultados.md`).
- **No inventar:** ni datos, ni endpoints de APIs, ni parámetros del broker. Si no está en la documentación oficial o en la spec, se pregunta.

## 5c. Trabajo por fases

- El proyecto avanza por las fases de `docs/ROADMAP.md`. **No se empieza una fase sin que la anterior esté aprobada por las dos personas** (aprobación registrada en el Issue de la fase).
- Toda entrega de una fase incluye: (1) qué se hizo, (2) código, (3) tests, (4) cómo verificarlo, (5) supuestos y riesgos, y (6) el **checklist de trading** de la plantilla de PR respondido.
- Si algo es ambiguo, el agente **pregunta antes de suponer**.

## 6. Decisiones (ADRs)

Cambios de lenguaje, librería principal, base de datos, proveedor de datos o broker, o un contrato roto → **ADR** en `docs/decisions/NNNN-titulo.md` (copiá `0000-plantilla.md`), en su propio PR, aprobado por ambos.

## 7. Idioma

- Documentación, Issues y PRs: **español**.
- Código (nombres de variables, funciones y tipos) y mensajes de commit: **inglés técnico** está bien, mientras sea consistente dentro del módulo.

## 8. Herramientas para agentes: graphify (opcional)

[graphify](https://github.com/Graphify-Labs/graphify) arma un grafo del repo para que los agentes lo consulten en lugar de leer archivos enteros, y así gasten menos tokens.
- **Es opcional y local.** El repo no trae su skill ni sus hooks. Cada persona decide si lo instala en su máquina (ver `CONTRIBUTING.md`) y **no commitea** lo que genera.
- **Sin graphify, los agentes trabajan normal** con sus herramientas de siempre. Ninguna regla de este archivo depende de él.
- Si lo tenés instalado, `graphify-out/` no se commitea (está en `.gitignore`). Se genera con `graphify update .`: es local, sin LLM, sin costo y tarda segundos.
- **graphify no reemplaza el paso 0.2:** el grafo refleja tu copia local, no `origin/main` ni los Issues y PRs abiertos. Para buscar duplicados, seguí usando `git grep ... origin/main` y revisando Issues y PRs.
- `/graphify .` dentro del asistente hace además una pasada semántica sobre los documentos que **sí consume tokens**. Usalo solo si hace falta.
