# Trabajar en paralelo sin perder avance

Somos dos personas y varios agentes de IA (sesiones en la nube y en computadoras propias) trabajando en el mismo repo. Las reglas generales están en `CLAUDE.md`. Esta guía suma dos cosas: **dónde se hace cada trabajo** y **cómo se cuida el cupo de IG**.

## 1. Qué se hace en cada lugar

| Lugar | Para qué | Qué NO se hace ahí |
|---|---|---|
| **Mac mini** (puesto en marcha con `docs/guides/puesta-en-marcha-local.md`) | Guardar los datos de verdad: PostgreSQL y Redis en Docker, cargas de IG, ingestas programadas | — |
| **Sesiones en la nube** y otras computadoras | Programar, correr tests, abrir PRs, investigar | **Cargas de datos que queremos conservar.** La base de una sesión en la nube se borra cuando la sesión termina. |

Antecedente: en la semana del 17 al 22-11-2026 se gastaron unos 8.900 de los 10.000 puntos de IG cargando diarias en una sesión en la nube (#102). Esos datos no llegaron al Mac mini.

## 2. El cupo de IG

- **10.000 puntos de históricos por semana**, por cuenta (`config/ig.toml`). Lo comparte **todo** lo que usa la cuenta demo, en cualquier computadora o sesión. No se puede ampliar.
- El **streaming** de IG no está habilitado para la cuenta demo: `CHART` responde "Insufficient permissions" y `PRICE` responde "Invalid group" (medido el 20 y el 22-11-2026). Las velas solo se consiguen por REST, con cupo. Ver `docs/research/fuentes-datos-gratis-2026-11.md`.

Reglas:
1. **Cargas grandes** (más de unos 100 puntos) **solo en el Mac mini**.
2. **En la nube, solo pruebas chicas**: un día o una semana de datos. Nunca `ig_live` ni cargas de años.
3. **Antes de una carga, mirar el cupo; después, anotarlo.** Cada carga muestra al final el `cupo semanal restante`. Se deja un comentario en el Issue de la carga con lo que se cargó y el cupo que quedó.
4. **El dato ya guardado no se vuelve a pedir.** Si falta algo, primero se revisa la base del Mac mini. Desde #137, `ig_backfill` e `ig_backfill_daily` lo hacen solas: miran la base y le piden a IG solo lo que falta.

## 3. Cómo elige tarea un agente

Es el paso 0 de `CLAUDE.md`, resumido:
1. `git fetch origin` y buscar si ya existe lo que vas a hacer (`git grep ... origin/main`, Issues y PRs abiertos).
2. Elegir un Issue **sin asignar y sin `in-progress`**. Si está tomado, no se toca.
3. Reclamarlo: asignártelo y ponerle `in-progress`. Si tu cuenta no puede asignar, dejá un comentario en el Issue avisando que lo tomás.
4. Una rama por Issue, un PR por Issue, con la plantilla completa y `Closes #N`.
5. El merge lo hace el dueño o su agente con la delegación de `CLAUDE.md` §2. Ningún otro agente mergea.

## 4. Qué tareas no gastan cupo

Son buenas para trabajar en paralelo desde la nube. La lista se actualiza en el Issue de la fase en curso.

| Issue | Qué es |
|---|---|
| #122 | 4b: interpretación de noticias (ajuste de −2 a +2) como evento |
| #79 | Backtest: tipo de referencia diario (€STR del BCE) en vez de constante |
| #25 | Spec: psicología del mercado como capa de contexto (documento) |

**Solo en el Mac mini:** el resto de las diarias (#102) y la programación del sentimiento y de las velas en vivo (#124).

## 5. Dónde está el estado de los datos

En los comentarios de #102 (velas diarias) y #124 (ingestas programadas). No se repite en este archivo porque cambia todas las semanas.
