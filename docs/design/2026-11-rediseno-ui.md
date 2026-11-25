# Rediseño de la UI: vidrio solo en el inicio, estilo propio por sección e intro animada

Issue: #581 · Estado: **propuesta** (no implementa nada; la implementación va en PRs por etapas, sección 5).
Fecha: 25-11-2026.

> Pedido del dueño (25-11-2026), textual: "no me gusto que todo este construido alrededor el mismo
> tipo de Liquid Glass Panel. Me parecia buena idea solo para elegir el tipo de estrategia o qcy
> cosas mas simples y despues entrar a diferentes secciones que tienen su propio estilo. Tipo para
> los resumenes y resultados y P/L esta bueno pero para lo demas como que no me gusto mucho."

Este documento propone **qué** cambia y **por qué**. No toca código. Todo lo que sigue respeta las
reglas de siempre: la UI solo muestra lo que manda el motor (no calcula indicadores, P/L, riesgo ni
señales), todo es simulación (paper) y ningún texto promete ganancias.

---

## 1. Diagnóstico: cómo está hoy

### 1.1 Un solo material para todo

- `ui/src/index.css` define **un solo juego de tokens** (grafito con plata, oscuro por defecto y
  claro con `data-theme="light"`) y dos utilidades de vidrio: `glass` (fondo translúcido +
  `backdrop-filter: blur(22px)` + borde y brillo) y `glass-strong`.
- `ui/src/App.tsx` monta, debajo de todo, `HalftoneWallpaper` (un shader en canvas que se mueve
  siempre) y encima `GlassShell` (`ui/src/components/ui/glass-shell.tsx`): barra superior de
  vidrio, dock lateral de vidrio, escenario de vidrio y el panel para **ajustar el vidrio**
  (`glass-settings.ts`).
- Dentro del escenario, **casi cada bloque de cada vista es otra tarjeta `.glass`** con
  `rounded-[20px]`. Conteo actual (`grep "glass"` en `ui/src`):

| Dónde | Qué es de vidrio |
|---|---|
| Armazón (`glass-shell.tsx`, 20 usos) | barra, dock, escenario, búsqueda, popovers, ajustes del vidrio |
| Terminal (`views/terminal.tsx`, `trading-terminal.tsx`, `watchlist.tsx`, `chart-draw-bar.tsx`) | franja de cuenta, panel de riesgo, mercados, barra de dibujo, cada gráfico, ticket, posiciones |
| Gráficos y Multigráfico (`views/multichart.tsx`, `views/index.tsx`) | barra de controles, cada celda de gráfico, panel lateral |
| Performance (`views/performance.tsx`, `run-detail.tsx`) | cada `Card`: corridas, estadísticas, fondo, aprendizaje, modelo |
| Panel (`views/panel.tsx` → liquidez, contexto, operación) | cada sección, `ops-status.tsx` |
| Fondo, Propuestas, Glosario (`views/phase10.tsx`, 7 usos) | KPIs, tablas, buscador, **cada término del glosario** |
| Noticias (`glass-folder.tsx`) | carpetas de vidrio 3D |
| Varios | ingreso (`sign-in.tsx`), cargador (`dot-loader.tsx`), tooltip del mapa de calor, botones `glass-strong` |

### 1.2 Qué se ve (capturas de los informes de QA)

- **Terminal** (`qa-terminal/shots/t1.png`): tarjetas de vidrio dentro de un escenario de vidrio
  dentro de un fondo animado. Tres capas de desenfoque detrás de precios y órdenes. Los bordes
  redondeados de 20 px y el relleno de 16 px se comen ancho útil: el ticket, los mercados y el
  gráfico compiten por espacio, y el gráfico (lo más importante) queda angosto.
- **Performance** (`qa-views/shots/perf.png`): la tabla de modelos (log loss, Brier, AUC) y los
  textos largos sobre vidrio gris; la jerarquía se pierde porque todo tiene el mismo peso visual.
- **Noticias** (`qa-views/shots/noticias.png`): tres carpetas grandes de vidrio y nada para leer.
  Es lindo como objeto, pero no es un lector.

### 1.3 Problemas concretos

1. **Sin jerarquía**: si todo es vidrio, nada se destaca. El vidrio debería marcar "esto es un
   resumen o una puerta", no "esto es un bloque cualquiera".
2. **Legibilidad**: el texto chico (12–13 px) y los números sobre un fondo translúcido que se mueve
   pierden contraste según lo que pase detrás.
3. **Rendimiento**: `backdrop-filter` sobre decenas de tarjetas + el shader del fondo + gráficos en
   canvas = mucho trabajo de composición, sobre todo en la terminal y en el celular.
4. **Densidad**: la terminal necesita densidad (como una terminal profesional); el vidrio pide aire.
5. **Contraste a revisar**: en el tema claro, `--chart-up: #13865c` sobre `--background: #e7e9ec`
   da **3,76:1**: sirve para gráficos (mínimo 3:1 para elementos no textuales) pero **no** para
   texto chico (pide 4,5:1). Hoy se usa también en textos de P/L.

Lo que **sí** funciona y se conserva: el vidrio en el ingreso, los resúmenes de cuenta y P/L, el
fondo semitono como "ambiente" del inicio, las animaciones cortas de `transform`/`opacity`, el
respeto por `prefers-reduced-motion` y `prefers-reduced-transparency`, y los toques de 44 px en el
celular.

---

## 2. Arquitectura de información

### 2.1 Mapa nuevo

```
Intro (solo la primera vez)
  └─ Ingreso
       └─ INICIO (hub, con vidrio)
            ├─ Terminal           — oscuro, denso, sin vidrio
            ├─ Gráficos           — lienzo a pantalla completa
            │    └─ Multigráfico
            ├─ Investigación      — editorial (hoy: Performance + Propuestas + Liquidez + Glosario)
            ├─ Noticias / Contexto — lector
            ├─ Fondo              — resumen con vidrio (es P/L y resultados)
            ├─ Diario             — libro de registro
            └─ Trofeos            — tablero de logros sobrio
```

Las vistas actuales se reubican sin perder ninguna: `#panel` pasa a ser el **Inicio**;
`#performance`, `#propuestas`, `#liquidez` y `#glosario` quedan dentro de **Investigación** (con
sus hashes de siempre, para no romper enlaces ni tests); `#noticias` suma el contexto de mercado
(sentimiento y análogos, hoy en el Panel); `#operaciones` es el **Diario**; `#competencia` son los
**Trofeos**. Las pestañas principales (`MAIN_TABS`) pasan a ser Inicio · Terminal · Gráficos ·
Investigación, y el gesto del trackpad sigue funcionando entre ellas.

### 2.2 Cada sección y su porqué

**Inicio (hub) — vidrio.** Es la única pantalla "de vidrio" completa, porque es la que el dueño
dijo que le gusta: elegir a dónde ir y ver resúmenes.

- Arriba: franja de cuenta en simulación (capital, P/L abierto, P/L del día, margen, pérdida del
  día usada), **tal como la manda el motor**. Etiqueta fija "Simulación (paper)".
- Centro: una grilla tipo *bento* con las **puertas** a cada sección. Cada puerta es una tarjeta de
  vidrio con el color de acento de su sección, un dato vivo chico (por ejemplo "3 propuestas
  esperando", "última corrida: 24 nov") y un atajo de teclado.
- Elegir estrategia: un selector de la estrategia activa en simulación (solo cambia qué se muestra;
  la selección y validación las hace el motor).
- Abajo: resultados recientes (últimas corridas, P/L de la semana en simulación) con su aviso
  "resultados del pasado, no promesas".
- El fondo semitono animado queda **solo acá** (y en el ingreso). En las secciones, fondo liso.

**Terminal — denso y oscuro, sin vidrio.** Una terminal se usa durante horas mirando números que
cambian: necesita máxima densidad, contraste estable y cero distracciones.

- Fondo casi negro liso, paneles separados por **líneas de 1 px** (no tarjetas), esquinas de 4 px,
  relleno de 8 px. Paneles redimensionables (mercados | gráfico | ticket; abajo posiciones,
  órdenes y actividad).
- Números en **monoespaciada** con cifras tabulares; etiquetas en mayúsculas chicas.
- Acento **ámbar** solo en el cromo (pestaña activa, foco, la etiqueta "SIMULACIÓN"); el verde y el
  rojo siguen reservados para la dirección del precio.
- Paleta de comandos (Ctrl/⌘ K) como forma principal de moverse: instrumento, timeframe, vista.
- En el tema claro no se fuerza el oscuro: usa un gris muy claro liso con las mismas reglas (sin
  vidrio), para respetar la preferencia del usuario.

**Gráficos y Multigráfico — lienzo a pantalla completa.** El protagonista es el gráfico; todo lo
demás molesta.

- El gráfico ocupa todo el alto y el ancho disponible (sin escenario ni tarjetas).
- Controles mínimos en una barra fina flotante arriba (instrumento, timeframe, herramientas) que se
  atenúa cuando el mouse está quieto sobre el gráfico y vuelve al moverlo o con el teclado.
- La barra de dibujo pasa a un riel delgado a la izquierda.
- Multigráfico: grilla 1/2/4 sin huecos, separadores de 1 px, la celda activa con un borde de
  acento (azul de análisis).
- La única superficie "flotante" permitida es el popover de herramientas; ahí puede ir un vidrio
  liviano, porque flota **sobre** el gráfico.

**Investigación / Performance — editorial.** Acá se lee: hipótesis, corridas, walk-forward,
métricas de ML, propuestas. Tiene que parecer un informe bien diagramado, no un tablero.

- Fondo "papel" (claro cálido en tema claro, grafito tibio en oscuro), columna de lectura de
  ~68 caracteres para los textos, títulos en **serif**, cuerpo en sans.
- Tablas con filas cebra suaves, encabezado fijo, números alineados a la derecha en monoespaciada,
  ordenables, con la fila de referencia (por ejemplo B1) marcada.
- Gráficos estadísticos (curva de capital, distribución, calibración) con ejes y leyendas
  explícitos; siempre con el intervalo o la incertidumbre que manda el motor.
- Índice lateral pegajoso con las secciones (Corridas · Walk-forward · Modelo · Propuestas ·
  Liquidez · Glosario).
- Acento **violeta** (no se confunde con dirección ni con señal).

**Noticias / Contexto — lector.** Las noticias se leen como artículos.

- Lista a la izquierda (titular, fuente, **hora de publicación real** en UTC), lectura a la derecha
  con tipografía cómoda (18 px, interlínea 1,6).
- Las carpetas de cada miembro pasan a ser filtros (chips) arriba de la lista; la carpeta 3D queda
  como detalle chico del encabezado, no como pantalla entera.
- El contexto de mercado (sentimiento y análogos del motor de Python) va como columna lateral
  "Contexto" con sus probabilidades e incertidumbre.
- Acento **verde azulado**.

**Fondo — resumen con vidrio.** Es P/L y resultados: justo lo que el dueño dijo que sí le gusta
con vidrio. Se ve como una extensión del Inicio: tarjeta grande de valor de la unidad, curva del
fondo y tabla de miembros (la tabla en superficie lisa dentro de la tarjeta, para que se lea).

**Diario — libro de registro.** Un diario de operaciones es una lista cronológica que se recorre y
se anota. Mismo lenguaje editorial que Investigación (papel, serif en fechas), pero en formato de
línea de tiempo: cada operación simulada es una entrada con fecha UTC, instrumento, motivo,
resultado del motor y la nota del miembro. Acento violeta (es parte de "aprender").

**Trofeos — tablero de logros sobrio.** Los trofeos motivan **proceso** (disciplina, respetar el
stop, completar el diario), no ganancias. Grilla de insignias con estados (logrado / en curso /
bloqueado), tabla del grupo y un acento **dorado** contenido. Sin confeti ni lenguaje de casino.

### 2.3 Tokens compartidos

Una sola app: mismas fuentes, misma escala, mismos radios y misma curva de movimiento. Cada sección
cambia **solo** tres cosas: superficie, densidad y acento. Se implementa con un atributo
`data-section="terminal|charts|research|news|hub|fund|journal|trophies"` en el contenedor de la
vista, que redefine unas pocas variables.

**Tipografía**

| Token | Valor | Uso |
|---|---|---|
| `--font-sans` | Manrope Variable (ya está) | interfaz y cuerpo |
| `--font-mono` | JetBrains Mono o IBM Plex Mono (OFL) | números de la terminal, tablas, precios |
| `--font-serif` | Source Serif 4 (OFL) | títulos de Investigación, Diario y Noticias |

Escala (px): 11 · 12 · 13 · 14 · 16 · 18 · 22 · 28 · 36 · 48. Cifras tabulares en todo número.

**Espaciado y forma**

| Token | Hub / Fondo | Terminal / Gráficos | Investigación / Noticias / Diario |
|---|---|---|---|
| `--density` (relleno base) | 16 px | 8 px | 16–24 px |
| `--radius` | 20 px | 4 px | 8 px |
| separación | tarjetas con aire | líneas de 1 px | reglas finas y espacio en blanco |
| superficie | `.glass` | lisa | lisa "papel" |

Escala de espaciado base 4 px (4, 8, 12, 16, 24, 32, 48).

**Color.** Se conservan los tokens de gráficos (`--chart-*`): la dirección del precio es verde/rojo
en **toda** la app. Los acentos de sección son solo para el cromo (foco, pestaña activa, títulos de
sección, bordes de la celda activa), nunca para datos.

| Sección | Superficie oscuro | Acento oscuro | Superficie claro | Acento claro |
|---|---|---|---|---|
| Inicio / Fondo | `#101113` + vidrio | plata `#d9dde3` (13,9:1) | `#e7e9ec` + vidrio | `#23262b` (12,5:1) |
| Terminal | `#0b0c0e` | ámbar `#f0b429` (10,5:1) | `#f3f4f6` | `#8a5a00` (5,4:1) |
| Gráficos | `#0b0c0e` | azul `#9fb4ff` (9,7:1) | `#f3f4f6` | `#3a55c9` (5,7:1) |
| Investigación / Diario | `#151619` | violeta `#b7a6ff` (8,5:1) | `#faf8f4` | `#5b3fb0` (7,1:1) |
| Noticias / Contexto | `#121314` | verde azulado `#6cc9b5` (9,4:1) | `#f6f2ea` | `#0d6e5f` (5,5:1) |
| Trofeos | `#101113` | dorado `#e6c35c` (11,1:1) | `#e7e9ec` | `#7a5b00` (5,2:1) |

Contrastes calculados con la fórmula de WCAG 2.x contra la superficie de cada fila. Texto
principal: `#d7dbe0`–`#e9e6df` sobre oscuros (≥ 14:1) y `#15171a`–`#1d1c1a` sobre claros
(≥ 15:1); texto secundario ≥ 6:1 en todos los casos.

**Accesibilidad AA (claro y oscuro)**

- Todo texto ≥ 4,5:1 (≥ 3:1 si es ≥ 18,66 px en negrita o ≥ 24 px); bordes de controles y marcas
  de gráficos ≥ 3:1.
- **Corrección propuesta**: en el tema claro, el verde de texto pasa de `#13865c` a `#0f7a53`
  (4,9:1 sobre `#f3f4f6`, 5,0:1 sobre `#faf8f4`). El rojo `#c2393c` ya cumple (4,8–5,0:1). En los
  gráficos puede seguir el verde actual.
- La dirección nunca se comunica **solo** con color: signo (+/−) y flecha siempre.
- Foco visible con el acento de la sección y `outline-offset: 3px` (ya existe).
- `prefers-reduced-transparency` y `prefers-contrast: more` ya apagan el vidrio: se mantiene.
- `prefers-reduced-motion`: se mantiene el apagado global.
- Toques de 44 px en el celular (ya existe). En la terminal densa se compensa con un modo
  "cómodo" automático en pantallas táctiles.
- Navegación completa por teclado; la paleta de comandos tiene roles ARIA de *combobox/listbox*.

---

## 3. Componentes candidatos de 21st.dev

Búsqueda hecha en la web pública de 21st.dev (solo lectura, 25-11-2026). No tenemos el MCP de
21st.dev; además, el comando de instalación de cada página
(`npx shadcn@latest add "https://21st.dev/r/...?api_key=$API_KEY_21ST"`) **pide una clave del
dueño**. Por eso la propuesta es: preferir componentes cuyo origen es una librería pública con
licencia MIT (shadcn/ui, Origin UI/coss, Kibo UI, ReUI, ShadcnSpace) y **reescribirlos** en
`ui/src/components/ui/` con nuestros tokens, como ya se hizo con el fondo semitono y el armazón
(#164). En este PR **no se copia código**.

Contexto técnico: la app usa React 19, Tailwind v4, `lucide-react` y `motion`; **no** tiene hoy
shadcn, Radix, Base UI ni TanStack. Cada dependencia nueva se pide en su PR, con su motivo.

| Para | Componente | Link | Autor / librería | Licencia | Qué resuelve | Dependencias |
|---|---|---|---|---|---|---|
| Tablas (Investigación, Diario, Fondo) | Table (TanStack) | [21st.dev/@originui/…/table](https://21st.dev/@originui/components/table/example-of-a-more-complex-table-made-with-tan-stack-table) | Origin UI | MIT | tabla ordenable, filtrable, con paginación y selección | shadcn table, `@tanstack/react-table` |
| Tablas (alternativa) | Data Table | [21st.dev/@shadcn/data-table](https://21st.dev/@shadcn/components/data-table) | shadcn/ui | MIT | la receta de referencia de shadcn sobre TanStack | `@tanstack/react-table` |
| Tablas (alternativa) | Table | [21st.dev/@haydenbleasel/data-table](https://21st.dev/@haydenbleasel/components/data-table) | Kibo UI (Hayden Bleasel) | MIT | tabla con agrupado y estado | `@tanstack/react-table`, `zustand`, `lucide-react` |
| Terminal (mercados) | Financial Markets Table | [21st.dev/@isaiahbjork/financial-markets-table](https://21st.dev/@isaiahbjork/components/financial-markets-table) | Isaiah | **a confirmar** (la página no muestra licencia) | idea de fila de mercado con mini-gráfico | `framer-motion`, `next-themes` |
| Terminal / todo | Command (paleta ⌘K) | [21st.dev/@wensity/command](https://21st.dev/@wensity/components/command) | Wensity Labs | MIT | búsqueda instantánea, grupos, atajos y páginas anidadas | `@base-ui/react`, `@tabler/icons-react`, `clsx`, `tailwind-merge` |
| Terminal / todo (alternativa) | Command Palette | [21st.dev/@ebnsina/command-palette](https://21st.dev/@ebnsina/components/command-palette) | ebnsina | MIT | diálogo ⌘K con acciones agrupadas por palabra clave | `@hugeicons/core-free-icons` (cambiable por lucide) |
| Terminal (paneles) | Resizable | [21st.dev/@sean0205/resizable](https://21st.dev/@sean0205/components/resizable/handle) | ReUI (Sean Hello) | MIT | paneles redimensionables accesibles con teclado | `react-resizable-panels`, `lucide-react` |
| Pestañas (Investigación, Noticias) | Tabs (subrayado con íconos) | [21st.dev/@coss.com/tabs](https://21st.dev/@coss.com/components/tabs/underline-with-icons) | coss (Origin UI) | MIT | pestañas accesibles con subrayado | `@base-ui/react` |
| Pestañas (Terminal, alternativa) | Vercel Tabs | [21st.dev/@yadwinder/vercel-tabs](https://21st.dev/@yadwinder/components/vercel-tabs) | Yadwinder Singh | MIT | pestañas compactas con resaltado que se desliza | no declara |
| Barra lateral | Sidebar | [21st.dev/@shadcn/sidebar](https://21st.dev/@shadcn/components/sidebar) | shadcn/ui | MIT | barra lateral colapsable a riel de íconos, móvil incluido | `@radix-ui/react-slot`, `class-variance-authority`, `lucide-react` |
| Barra lateral (alternativa) | Sidebar | [21st.dev/@wensity/sidebar](https://21st.dev/@wensity/components/sidebar) | Wensity Labs | MIT | riel de íconos, secciones, insignias, ítem activo animado | `@base-ui/react`, `framer-motion`, `@tabler/icons-react` |
| Inicio (riel de vidrio) | Glass Sidebar | [21st.dev/@deepraj21/glass-sidebar](https://21st.dev/@deepraj21/components/glass-sidebar) | deepraj21 | MIT | navegación de vidrio flotante con resaltado deslizante | `radix-ui`, `@hugeicons/*` |
| Inicio (bento de puertas) | Stats Bento | [21st.dev/@uilayout.contact/stats-bento](https://21st.dev/@uilayout.contact/components/stats-bento) | UI Layouts | MIT | grilla bento de métricas de tamaños distintos | no declara |
| Inicio (bento financiero) | Finance Bento | [21st.dev/@romanvasnev/finance-bento](https://21st.dev/@romanvasnev/components/finance-bento) | Vasnev.art | MIT | KPIs, gauge y curva sin librería de gráficos | ninguna |
| Inicio (bento, alternativa) | Financial Bento Grid | [21st.dev/@uiable/block-bento-5](https://21st.dev/@uiable/components/block-bento-5) | uiable | MIT | bento fintech con gráfico de área y acciones | `recharts`, `@tabler/icons-react` (no se suma recharts: los gráficos ya existen) |
| Tarjetas de métricas | Numbers Stats Grid | [21st.dev/@shadcnui-blocks/stats-06](https://21st.dev/@shadcnui-blocks/components/stats-06) | Shadcn UI Blocks | MIT | grilla de cifras con ícono | `lucide-react` |
| Tarjetas de métricas (alternativa) | Stats | [21st.dev/@meschacirung/stats](https://21st.dev/@meschacirung/components/stats/stats-metric) | Tailark (Méschac Irung) | **a confirmar** | cifra grande + etiqueta | no declara |
| Tarjetas de métricas (alternativa) | Stats card with area chart | [21st.dev/@ephraimduncan/…](https://21st.dev/@ephraimduncan/components/stats-cards-with-links/stats-card-with-area-chart) | Ephraim Duncan | **a confirmar** | KPI con mini-área y link | `clsx`, `tailwind-merge` |
| Pasos (Propuestas, walk-forward) | Stepper | [21st.dev/@originui/stepper](https://21st.dev/@originui/components/stepper/with-reversed-inline-titles-and-descriptions) | Origin UI | MIT | pasos con título y descripción | `lucide-react`, `@radix-ui/react-icons` |
| Pasos (alternativa) | Animated Progress Stepper | [21st.dev/@shadcnspace/stepper-03](https://21st.dev/@shadcnspace/components/stepper-03) | ShadcnSpace | MIT | progreso animado con resorte y check | `motion`, `lucide-react` (ya las tenemos) |
| Hero del Inicio (referencia) | Editorial Collage Hero | [21st.dev/@felipemenezes098/hero-04](https://21st.dev/@felipemenezes098/components/hero-04) | felipemenezes098 | **a confirmar** (sin licencia en la página) | titular serif + bajada + llamadas | `motion`, `react-wrap-balancer` |

**Recomendación** (la combinación más chica que cubre todo):

1. **Paleta de comandos**: Wensity *Command* (MIT). Reemplaza la búsqueda actual de `glass-shell`.
2. **Tablas**: Origin UI *Table* sobre TanStack (MIT). Una sola tabla para Investigación, Diario,
   Fondo y Trofeos; la de mercados de la terminal se arma con la misma base (no la de Isaiah, cuya
   licencia está a confirmar).
3. **Paneles de la terminal**: ReUI *Resizable* (MIT) → `react-resizable-panels`.
4. **Pestañas**: coss *Tabs* (MIT).
5. **Barra lateral**: shadcn *Sidebar* (MIT) para las secciones; *Glass Sidebar* solo como idea
   para el riel del Inicio.
6. **Inicio**: *Stats Bento* + *Finance Bento* (MIT) como referencia de grilla; las cifras vienen
   del motor.
7. **Pasos**: ShadcnSpace *Animated Progress Stepper* (MIT; usa `motion` y `lucide`, que ya están).

Para no sumar tres librerías de primitivas distintas, se elige **una**: `@base-ui/react` (la usan
Command, Tabs y Sidebar de Wensity) **o** Radix (shadcn). Propuesta: **Base UI**, y adaptar el
Sidebar de shadcn a Base UI al reescribirlo. Íconos: siempre `lucide-react` (se reemplazan Tabler
y Hugeicons al reescribir). Se descartan los que tienen licencia "a confirmar" hasta que el dueño
la verifique en la página del componente con su cuenta. Si el dueño crea la clave de 21st.dev, se
puede instalar con el CLI y comparar contra la reescritura; la clave va en `.env`, nunca en el
repo.

---

## 4. Intro animada original: "El mercado espera"

Inspirada en una **mecánica** de juego conocida (el tiempo solo avanza cuando el jugador se mueve),
llevada al mercado: **el mercado solo avanza cuando te movés**. No usa el nombre, el arte, los
colores, la tipografía ni los textos del juego: es un gráfico de velas en grafito, con un único
acento cian helado, en nuestra tipografía (Manrope + la mono de la terminal).

### 4.1 Guion

| Momento | Qué pasa | Duración |
|---|---|---|
| 0. Congelado | Pantalla grafito casi negra. Un gráfico de velas **detenido** a mitad de pantalla, en grises; la última vela, a medio formar, en cian. Arriba a la derecha, chico: "Saltar (Esc)". Abajo, en mono: `el mercado espera`. | hasta que el usuario se mueva (máx. 4 s) |
| 1. Avanzar | Al mover el mouse, hacer scroll o deslizar el dedo, el tiempo corre **proporcional al movimiento**: aparecen velas nuevas de izquierda a derecha y la cámara se desliza. Si se queda quieto, todo se congela otra vez (las velas "respiran" con un leve parpadeo del cursor de tiempo). | ~3–5 s de movimiento acumulado |
| 2. Frase | Tres frases cortas, una por cada tramo del recorrido, en mono: `el tiempo avanza cuando vos avanzás` · `cada vela, solo cuando cierra` · `simulación, no promesas`. | se reparten en el tramo 1 |
| 3. Cierre | La última vela cierra; el gráfico se aleja y se funde con el ingreso. Marca "CFDsTrader" y "Simulación (paper)". | 0,6 s |

Si en 4 s el usuario no se mueve, el tiempo empieza a correr solo, despacio (para quien no entienda
la mecánica o use lector de pantalla). Duración total esperada: **6–9 s**; tope duro **12 s**.

Las velas son una **serie dibujada a mano, fija y ficticia** (un archivo de datos de la intro), no
precios reales ni del motor: así la intro no calcula nada, no depende del gateway y no sugiere
ningún resultado. La frase "cada vela, solo cuando cierra" refuerza a propósito la regla del
proyecto (solo velas cerradas). **Ningún texto promete ganancias** ni muestra cifras de dinero.

### 4.2 Cuándo aparece

- Solo la **primera vez** en ese navegador, antes del ingreso. Reemplaza al cargador de puntos en
  esa visita; la transición del túnel (#414) sigue después del ingreso como hoy.
- Se guarda `cfds.intro.seen = 1` en `localStorage` (envuelto en `try/catch`: si el navegador lo
  bloquea, la intro no se muestra para no repetirla en cada carga).
- En Ajustes: "Ver la intro otra vez".
- Se salta con **Esc**, **Enter**, **Espacio**, un clic en "Saltar" o un toque largo. Al saltar,
  la preferencia se guarda igual.
- En desarrollo, `VITE_INTRO_PREVIEW=1` la fuerza (como `VITE_PORTAL_PREVIEW`); los tests la
  desactivan por defecto.

### 4.3 Rendimiento

- **Un solo `<canvas>` 2D** con `requestAnimationFrame`; nada de DOM por vela ni de layout por
  cuadro. Las frases son un `<p>` con `opacity`.
- Escuchas `pointermove`, `wheel` y `touchmove` **pasivas**; el movimiento se acumula en un número
  y el cuadro lo consume (sin trabajo dentro del evento).
- Densidad de píxeles limitada a 2; ~120 velas precalculadas en coordenadas, solo se dibujan las
  visibles.
- Se pausa con la pestaña oculta (`visibilitychange`); temporizador de respaldo como en
  `portal-transition.tsx`.
- Código propio, sin librerías nuevas; carga diferida (`import()`), así no pesa en las visitas
  siguientes. Objetivo: < 8 kB comprimido y 60 fps en un portátil común.
- Mientras corre la intro, el fondo semitono no se monta.

### 4.4 Accesibilidad

- `prefers-reduced-motion: reduce`: **no** hay recorrido. Se muestra el gráfico quieto con la frase
  final durante 1,2 s, fundido simple, y sigue al ingreso.
- `role="dialog"` con `aria-label="Introducción"`, foco en el botón "Saltar", y una región
  `aria-live="polite"` que lee las tres frases. Lector de pantalla: la intro se resume en una sola
  frase y se puede saltar de inmediato.
- Contraste: gris `#8a8f96` sobre `#0a0a0b` = 6,1:1; acento cian `#7fd6e8` = 12,0:1; texto
  principal `#e8e8e8` = 16,2:1.
- Sin destellos: ningún cambio de luminosidad de pantalla completa más de 3 veces por segundo
  (WCAG 2.3.1).
- Teclado: las flechas ← → también hacen avanzar el tiempo (alternativa al mouse).

---

## 5. Plan de implementación por etapas

Cada etapa es **un PR chico** (< 400 líneas sin tests), con su Issue hijo de #581 y capturas a
1400 px y 375 px, en claro y oscuro.

| Etapa | PR | Qué hace | Toca |
|---|---|---|---|
| 1 | Tokens por sección | Agrega `--font-mono`, `--font-serif`, `--density`, `--radius`, `--accent` y `data-section`; corrige el verde de texto del tema claro. Sin cambios visibles fuera de eso. | `index.css`, `package.json` (fuentes OFL) |
| 2 | **Inicio (hub)** | `#panel` pasa a Inicio con bento de puertas, franja de cuenta y resultados. El vidrio y el fondo semitono quedan solo acá. | `views/panel.tsx`, `nav.ts`, `App.tsx` |
| 3 | **Terminal** | Sin vidrio, líneas de 1 px, mono, paneles redimensionables (`react-resizable-panels`). | `views/terminal.tsx`, `trading-terminal.tsx`, `watchlist.tsx` |
| 4 | Paleta de comandos | Reemplaza la búsqueda del armazón (agrega `@base-ui/react`). | `glass-shell.tsx` → `command-palette.tsx` |
| 5 | Gráficos y Multigráfico | Lienzo completo, barra flotante que se atenúa, riel de dibujo. | `views/index.tsx`, `views/multichart.tsx`, `chart-draw-bar.tsx` |
| 6 | Investigación | Editorial: índice lateral, tablas TanStack (agrega `@tanstack/react-table`), serif. Reúne Performance, Propuestas, Liquidez y Glosario sin cambiar sus hashes. | `views/performance.tsx`, `phase10.tsx`, `run-detail.tsx` |
| 7 | Noticias / Contexto | Lector con lista y columna de contexto. | `views/index.tsx` (NewsView), `market-context.tsx`, `glass-folder.tsx` |
| 8 | Fondo, Diario y Trofeos | Fondo con vidrio de resumen; Diario en línea de tiempo; Trofeos sobrios. | `phase10.tsx`, `trade-journal.tsx`, `trophy-card.tsx` |
| 9 | Intro "El mercado espera" | Componente nuevo en canvas, preferencia guardada, ajustes. | `intro.tsx` nuevo, `App.tsx` |
| 10 | Limpieza | Quita el panel de "ajustar el vidrio" si ya no tiene sentido y los `.glass` que quedaron. | `glass-settings.ts`, `glass-shell.tsx` |

Cada PR que agregue un componente nuevo actualiza el *Registro de módulos* de
`docs/ARCHITECTURE.md`.

### Riesgos y cómo se cuidan

- **Romper tests**: hay 32 archivos de test en `ui/src` que buscan textos, roles y hashes. Regla
  para cada etapa: no cambiar textos ni roles accesibles sin actualizar el test en el mismo PR; los
  hashes viejos (`#panel`, `#performance`, …) siguen funcionando. `eslint`, `prettier`, `tsc` y
  `vitest` en verde antes de abrir el PR.
- **"La UI no calcula trading"** (CLAUDE.md §1): ningún componente nuevo calcula P/L, margen,
  indicadores, señales ni ordena por un criterio de trading propio. Las tablas ordenan **lo que
  llega** (ordenar una columna es presentación, no cálculo). El selector de estrategia del Inicio
  solo filtra lo que se muestra. Las velas de la intro son una ilustración fija.
- **Dependencias nuevas**: cada una (Base UI, TanStack Table, react-resizable-panels, dos fuentes
  OFL) en su PR, con su licencia. Si alguna supone una "librería principal" de la UI, se abre el
  ADR correspondiente (CLAUDE.md §6) antes de sumarla.
- **Licencias**: solo MIT o similar; los "a confirmar" no se usan hasta que el dueño los verifique.
  Al reescribir, se deja el crédito al autor y el link en el comentario del componente.
- **Rendimiento**: sacar `backdrop-filter` de la terminal y los gráficos debería **mejorar** los
  fps; se mide antes y después con el panel de rendimiento del navegador.
- **Coherencia**: el riesgo de que parezcan "varias apps" se cuida con los tokens compartidos
  (fuentes, escala, radios, curva de movimiento y colores de gráficos iguales en todas partes).
- **Mensajes**: los avisos "Simulación (paper)" y "resultados del pasado, no promesas" quedan
  visibles en cada sección, con el estilo de la sección.

---

## 6. Decisiones que necesita el dueño

1. ¿Aprueba el mapa (Inicio, Terminal, Gráficos, Investigación, Noticias/Contexto, Fondo, Diario,
   Trofeos) y que Performance, Propuestas, Liquidez y Glosario vivan dentro de Investigación?
2. ¿Base UI como única librería de primitivas (en vez de Radix)?
3. ¿Mono: JetBrains Mono o IBM Plex Mono? ¿Serif: Source Serif 4?
4. ¿La terminal respeta el tema claro (propuesta) o queda siempre oscura?
5. ¿Intro antes del ingreso, solo la primera vez (propuesta)?
6. ¿Crea la clave de 21st.dev para verificar las licencias "a confirmar"? (Opcional.)
