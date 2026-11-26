# Terminal profesional: sin vidrio, superficies lisas y densidad de terminal

Issue: #708 (eje 1 de la épica #707) · Fecha: 26-11-2026 · Reemplaza la regla de vidrio de
`2026-11-rediseno-ui.md` (#581) y `2026-11-componentes-21st.md` (#625).

> Pedido del dueño (26-11-2026), textual: «Me quiero des hacer del panel tipo liquid glass que tiene
> toda la pagina y ponerlo como si fuera una professional de verdad como esas terminales que
> cuestan millones. [...] No hace falta destruir toda la pagina pero si quiero un cambio general.»

## 1. Qué cambia y qué no

**Cambia el material, la densidad y el armazón. No cambia la estructura:** las mismas vistas,
hashes, rutas del gateway, tests de comportamiento y reglas de siempre (la UI no calcula nada de
trading, todo es simulación, ningún texto promete ganancias).

| Antes (#581) | Ahora (#708) |
|---|---|
| Vidrio en el Inicio, el ingreso y el Fondo (`data-glass`, `backdrop-filter`) | Sin vidrio en ninguna parte |
| Fondo semitono animado (shader en canvas) detrás del Inicio y el ingreso | Fondo liso casi negro |
| Grafito con plata; un acento distinto por sección (plata, ámbar, azul, violeta, verde azulado, dorado) | Paleta sobria: ámbar para el cromo, cian para lo informativo, dorado solo en Trofeos |
| Esquinas de 20–28 px en el Inicio y el Fondo, 8 px en Investigación | Esquinas de 2–4 px en toda la app |
| Números en mono solo en la Terminal | Números en mono en toda la app (`.tabular-nums`, `td`, `time`, `kbd`) |
| Paneles que aparecen escalonados (420 ms) | Fundido corto de la vista (140–160 ms), todo junto |
| Barra superior y dock en píldoras de vidrio | Barra fina con línea de comandos, barra de estado con relojes y frescura de datos (PR 2) |
| Inicio: tarjetas grandes (puertas) | Inicio: espacio de trabajo en paneles (PR 3) |

## 2. Principios

1. **Superficie lisa, regla de 1 px.** Una tarjeta es `--card` con borde `--border`; lo que flota
   (menús, diálogos) agrega una sombra corta (`--shadow`), nunca desenfoque.
2. **Densidad.** Relleno base de 8–12 px (`--density`), títulos chicos, sin aire decorativo. El
   gráfico y las tablas son lo importante.
3. **Números en mono con cifras tabulares**, alineados a la derecha en tablas.
4. **El color dice algo.** Verde y rojo solo para la dirección del precio (siempre con signo o
   flecha además del color). Ámbar para el control activo y el foco. Cian para lo informativo
   (hora, fuente, enlace). Nada de colores decorativos.
5. **Teclado primero.** ⌘K / Ctrl K sigue; se suman atajos de función y una línea de comandos
   (PR 2).
6. **Accesibilidad igual o mejor:** AA en ambos temas (verificado en `tokens.test.tsx`), axe sin
   fallas serias ni críticas, toques de 44 px y 375 px sin desplazamiento horizontal.

## 3. Tokens

| Token | Oscuro | Claro | Uso |
|---|---|---|---|
| `--background` / `--section-bg` | `#0a0b0d` | `#eef0f2` | fondo de la app |
| `--card` | `#0e1012` | `#f6f7f8` | paneles |
| `--surface` | `#121417` | `#ffffff` | campos, filas resaltadas |
| `--border` | blanco 12 % | negro 14 % | reglas de 1 px |
| `--rule` | blanco 22 % | negro 28 % | regla fuerte (encabezados, barra de estado) |
| `--foreground` | `#e4e6e9` (15,8:1) | `#14161a` (15,9:1) | texto |
| `--muted-foreground` | `#959ba3` (7,0:1) | `#4a5058` (7,1:1) | texto secundario |
| `--primary` / `--accent` | ámbar `#f0b429` (10,6:1) | `#7a5000` (6,2:1) | control activo, foco |
| `--info` | cian `#4fc3dc` (9,5:1) | `#0b6378` (6,0:1) | hora, fuente, enlaces |
| `--chart-up` / `--chart-down` | `#3fbf8c` / `#e0686a` | `#0a6643` / `#a52d30` | dirección (sin cambios) |

Contrastes contra `--background` con la fórmula de WCAG 2.x. Acento por sección: ámbar en Inicio,
Fondo y Terminal; cian en Gráficos, Investigación, Diario y Noticias; dorado en Trofeos. La
Terminal sigue siempre oscura.

Las clases `rounded-*` de Tailwind leen `--radius-xs` a `--radius-3xl` (1–4 px). `rounded-full`
queda para puntos de estado y avatares.

## 4. Plan de PRs

| PR | Qué | Archivos principales |
|---|---|---|
| 1 | Este documento, tokens y tema: sin vidrio ni fondo animado, paleta, esquinas, mono, movimiento corto | `index.css`, `App.tsx`, `tokens.test.tsx` |
| 2 | Armazón: barra fina con línea de comandos, navegación lateral en lista, barra de estado (relojes UTC, Nueva York, Londres y Tokio; frescura de las fuentes de datos del gateway), atajos F1–F9 y ayuda con `?` | `glass-shell.tsx`, `status-bar.tsx` |
| 3 | Inicio como espacio de trabajo: mercados, titulares, calendario y resultados de investigación en paneles, con las rutas que ya existen | `views/home*.tsx` |
| 4 | Limpieza: borrar el fondo animado, los ajustes del vidrio, `GLASS_VIEWS` y los nombres `.glass` | varios |

Cada PR actualiza las capturas de Playwright desde el artefacto de Linux del CI
(`docs/guides/pruebas-e2e-ui.md`).

## 5. Relojes y frescura (PR 2): qué se muestra y qué no

- Los relojes muestran la **hora local** de cada plaza (UTC, Nueva York, Londres, Tokio) con
  `Intl.DateTimeFormat` y la zona IANA. **No** dicen si el mercado está abierto: los horarios de
  sesión son configuración del motor (`config/`) y la UI no los calcula.
- La frescura de los datos sale tal cual del gateway (hora de la última vela o del último dato de
  cada fuente). La UI solo resta esa hora de la actual para decir «hace N min», como ya hace la
  lista de mercados.

## 6. Riesgos

- **Capturas.** Cambian todas: se regeneran en Linux desde el CI, nunca en macOS.
- **Contraste.** Los tokens nuevos se prueban en `tokens.test.tsx` en ambos temas y sobre las
  fichas al 10 % (sello EJEMPLO) y la tarjeta de un trofeo ganado.
- **Volver atrás.** Todo es CSS y armazón: revertir el PR alcanza.
