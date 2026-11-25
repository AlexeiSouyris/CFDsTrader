# Componentes de 21st.dev por sección

Issue: #625 · Sigue a: `docs/design/2026-11-rediseno-ui.md` (#581) · Fecha: 25-11-2026.

> Pedido del dueño (25-11-2026), textual: "quiero que utlices 21st dev mcp conection para encontrar
> elementos para nuestra app. La verdad que no me gusto que todo este construido alrededor el mismo
> tipo de Liquid Glass Panel. Me parecia buena idea solo para elegir el tipo de estrategia o qcy
> cosas mas simples y despues entrar a diferentes secciones que tienen su propio estilo. Tipo para
> los resumenes y resultados y P/L esta bueno pero para lo demas como que no me gusto mucho."

## 1. Qué falta después del rediseño

El rediseño (#581) ya dejó tokens por sección (`data-section`: acento, superficie, densidad, radio)
y sacó el vidrio de casi todas las vistas. Pero el vidrio se reemplazó por **otra tarjeta igual**:
la utilidad `.panel` (fondo `--card`, borde de 1 px, esquinas redondeadas). Resultado: Liquidez,
Performance, Propuestas, Diario y Trofeos siguen siendo una pila de cajas oscuras redondeadas que
se ven iguales. Cambió el material, no la forma.

Lo que falta es que **cada sección tenga su propia forma de mostrar las cosas**, no solo su color:

| Sección | Forma que le corresponde | Hoy |
|---|---|---|
| Inicio, elegir estrategia | vidrio: puertas y selectores simples | vidrio (bien) |
| Resúmenes, resultados, P/L | vidrio: cifras grandes | vidrio (bien) |
| Terminal | filas densas, líneas de 1 px, mono | tarjetas `.panel` chicas |
| Investigación / Liquidez | informe: secciones numeradas, reglas finas, tablas de datos | tarjetas `.panel` apiladas |
| Noticias | lector: lista + artículo | lista de tarjetas |
| Diario | línea de tiempo por día | tabla en tarjeta |
| Trofeos | grilla de insignias con estado | tarjetas |
| Ajustes | formulario por secciones | popover de vidrio |

## 2. Cómo se buscó y qué se usa

- Búsqueda con el **MCP de 21st.dev** (servidor `21st`, clave del dueño en su configuración local,
  nunca en el repo). Cuenta con acceso pago: búsquedas y lectura de código sin límite diario.
- La API de 21st **no informa la licencia** de los componentes. Por eso no se copia código tal
  cual: se toma el **patrón** (estructura, densidad, estados, accesibilidad) y se **reescribe**
  chico en `ui/src/components/ui/` con nuestros tokens, como ya se hizo con el armazón y el fondo
  (#164). El crédito y el link al original quedan en el comentario del componente.
- **Sin dependencias nuevas** en esta tanda: varios candidatos traen 2000+ líneas, TanStack,
  Recharts o librerías de íconos propias. Nosotros ya tenemos `motion`, `lucide-react` y nuestras
  tablas; con eso alcanza. Si alguna sección lo necesitara, va en su PR con versión exacta.
- Reglas que no cambian: la UI no calcula nada de trading (ni P/L, ni variaciones, ni estados de
  niveles: los muestra como llegan del motor), todo es simulación (paper) y ningún texto promete
  ganancias. axe sin fallas serias ni críticas y 375 px andando.

## 3. Lista corta por sección

Ids de 21st (`id`) para volver a pedirlos con `get_component`.

### Inicio y elegir estrategia — vidrio

| Elegido | Alternativa |
|---|---|
| **Glass Radio Group** — [@ravikatiyar162](https://21st.dev/@ravikatiyar162/components/glass-radio-group) · id 5408 | Comparing Radio Cards — [@dhxwhy](https://21st.dev/@dhxwhy/components/comparing-radio-cards) · id 44854 |

**Por qué:** es exactamente "elegir el tipo de estrategia": un grupo de opciones tipo píldora de
vidrio con la elegida resaltada, hecho con radios reales (teclado y lector de pantalla gratis). La
alternativa muestra diferencias entre opciones como chips; sirve si más adelante se comparan
estrategias, pero esas diferencias tendrían que venir calculadas del motor.

### Resúmenes, resultados y P/L — vidrio

| Elegido | Alternativa |
|---|---|
| **KPI Tile** — [@osagi](https://21st.dev/@osagi/components/kpi-tile) · id 40086 | Tweenly Chart KPI — [@mirukullacode](https://21st.dev/@mirukullacode/components/tweenly-chart-kpi) · id 39900 |

**Por qué:** cifra grande, cambio con flecha (no solo color), mini-línea y estados de carga y
vacío. Encaja con `metric-bento.tsx` y `animated-number.tsx` que ya existen. Ojo: el cambio y la
línea se muestran **solo si los manda el motor**; la UI no los calcula.

### Terminal — denso, sin vidrio

| Elegido | Alternativa |
|---|---|
| **Market Watchlist** — [@ssychui](https://21st.dev/@ssychui/components/market-watchlist) · id 20110 | Trading Number Ticker — [@shadcnspace](https://21st.dev/@shadcnspace/components/number-ticker-07) · id 29806 |

**Por qué:** una fila por instrumento, todo alineado como una sola tabla, sin tarjetas. Es la
densidad de una terminal profesional. Del *ticker* se toma solo el destello corto al cambiar el
precio (con `prefers-reduced-motion` apagado).

### Investigación / Liquidez — informe con tablas de datos

| Elegido | Alternativa |
|---|---|
| **Audit Log Table** — [@arihantcodes_1f7b8c4d](https://21st.dev/@arihantcodes_1f7b8c4d/components/audit-log-table) · id 29266 | Records Table — [@theshanelevine](https://21st.dev/@theshanelevine/components/records-table) · id 23604 |
| **Article With Author Sidebar** (diagramación) — [@olewandowski1](https://21st.dev/@olewandowski1/components/article-5) · id 29471 | — |

**Por qué:** la tabla elegida es densa, con borde de 1 px, encabezado fijo, valores en mono y el
estado como **punto + palabra** (no solo color). Es lo que piden los niveles y los FVG. La
diagramación de artículo da la forma de informe: secciones numeradas separadas por una regla fina
en vez de cajas. Del original (2185 líneas, con búsqueda, filtros y columnas redimensionables) se
toma solo el estilo; nuestras tablas ya ordenan y ya tienen sus tests.

### Noticias — lector

| Elegido | Alternativa |
|---|---|
| **Article With Author Sidebar** — [@olewandowski1](https://21st.dev/@olewandowski1/components/article-5) · id 29471 | News Card — [@ssychui](https://21st.dev/@ssychui/components/news-card) · id 20087 |

**Por qué:** dos columnas, texto a la izquierda y columna lateral pegajosa a la derecha: ahí va el
contexto de mercado (sentimiento y análogos con su incertidumbre). Del *News Card* se toma la línea
de fuente + **hora de publicación real** de cada titular.

### Diario — línea de tiempo

| Elegido | Alternativa |
|---|---|
| **Timeline** — [@cubby-ui](https://21st.dev/@cubby-ui/components/timeline) · id 28295 | Activity Timeline (agrupada por día) — [@olewandowski1](https://21st.dev/@olewandowski1/components/timeline-3) · id 28340 |

**Por qué:** eventos con fecha unidos por una línea, componible y sin dependencias. Un diario se
recorre en el tiempo; una tabla en una tarjeta no transmite eso. Se agrupa por día (UTC) como la
alternativa. *Trade Journal Table* ([@ssychui](https://21st.dev/@ssychui/components/trade-journal-table),
id 27124) queda descartado: pinta cada operación como "ganada o perdida", y el foco del Diario es el
proceso.

### Trofeos — grilla de insignias

| Elegido | Alternativa |
|---|---|
| **Achievement badge** — [@trophyso](https://21st.dev/@trophyso/components/achievement-badge) · id 12997 | Department Badge Grid (grilla con líneas finas) — [@olewandowski1](https://21st.dev/@olewandowski1/components/team-2) · id 28632 |

**Por qué:** insignia con estado (logrado / en curso / bloqueado) en distintos tamaños; en una
grilla dividida por líneas finas, sin tarjetas. Sobrio, sin confeti: premia proceso, no ganancias.

### Ajustes — formulario por secciones

| Elegido | Alternativa |
|---|---|
| **Settings Sidebar Layout** — [@felipemenezes098](https://21st.dev/@felipemenezes098/components/settings-2) · id 28366 | Form layout 02 — [@clevision](https://21st.dev/@clevision/components/shadcn-studio-form-layout-02) · id 46778 |

**Por qué:** hoy los ajustes son un popover de vidrio; con más opciones (tema, intro, vidrio,
fuente de datos) conviene una página con menú lateral y grupos con título y descripción.

## 4. Orden de los PRs

Otros agentes tienen PRs abiertos sobre la Terminal (#623), los Gráficos (#620), Noticias e
Investigación (#619), Diario y Trofeos (#622) y el armazón (#624). Para no pisarlos, se empieza
por lo que está libre y el resto espera a que esos se mergeen:

| Orden | Sección | Archivos | Espera a |
|---|---|---|---|
| 1 | **Liquidez** como informe | `views/liquidity.tsx`, `components/ui/liquidity-panels.tsx` | — |
| 2 | Investigación (Performance, Propuestas) | `views/performance.tsx`, `views/phase10.tsx` | #619 |
| 3 | Noticias como lector | `views/news.tsx` | #619 |
| 4 | Diario en línea de tiempo | `views/journal.tsx`, `components/ui/trade-journal.tsx` | #622 |
| 5 | Trofeos en grilla | `views/trophies.tsx` | #622 |
| 6 | Terminal densa | `components/ui/watchlist.tsx`, `trading-terminal.tsx` | #623 |
| 7 | Inicio: selector de estrategia y KPI | `views/home*.tsx` | #619 |
| 8 | Ajustes | `glass-shell.tsx` | #624 |

Cada PR: un Issue hijo de #625, menos de 400 líneas sin tests, capturas antes/después descritas
(1400 px y 375 px), y el componente nuevo en el *Registro de módulos* si corresponde.
