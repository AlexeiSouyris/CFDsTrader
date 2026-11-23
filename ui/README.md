# Interfaz de CFDsTrader: prototipo integrado

**Maqueta con datos de ejemplo** (Issues #30, #164 y #171). No se conecta al motor ni a IG, y **no calcula nada**: la UI solo muestra lo que envía el motor (CLAUDE.md §1). La app real es la fase 10.

## Qué hay

Una sola app (no una galería de piezas sueltas): carga → ingreso (email, contraseña y código 2FA) → armazón de vidrio con nueve vistas y una página 404.

| Vista (`#hash`)            | Qué muestra                                                                                                                                                                                                                           |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Panel (`#panel`)           | KPIs del fondo, comparación de índices con mini curvas, mapa de mercado, últimas operaciones y la tabla de la competencia                                                                                                             |
| Gráficos (`#graficos`)     | Velas o línea con volumen, RSI 14 y MACD 12·26·9 en paneles, y la profundidad del mercado                                                                                                                                             |
| Liquidez (`#liquidez`)     | Velas de 15m con los niveles de liquidez (EQH/EQL, PDH/PDL, PWH/PWL y sesiones) y su estado, los FVG abiertos y llenos, las estadísticas con IC 95 % y el índice de desequilibrio ("aproximación desde el precio"). Spec §5.4.1, #231 |
| Propuestas (`#propuestas`) | Las propuestas del motor (campos de `proposal.v1`), con su desglose, la probabilidad de §13, cuándo vencen y aprobar o rechazar (maqueta, solo el responsable)                                                                        |
| Noticias (`#noticias`)     | Una carpeta por miembro: todos ven todas, cada uno agrega solo en la suya                                                                                                                                                             |
| Diario (`#operaciones`)    | Diario de operaciones simuladas, con filtro por período                                                                                                                                                                               |
| Fondo (`#fondo`)           | Valor de la unidad, y unidades, aportes, valor y P&L de cada miembro, tal como llegan                                                                                                                                                 |
| Trofeos (`#competencia`)   | Trofeo del día (plata) y de la semana (oro), y la tabla del grupo                                                                                                                                                                     |
| Glosario (`#glosario`)     | Buscador de todos los términos de `docs/GLOSARIO.md`                                                                                                                                                                                  |

El fondo es un flujo en semitono gris grafito y plata (WebGL, lento, quieto con `prefers-reduced-motion`). El vidrio se ajusta desde la barra superior (desenfoque y opacidad), y hay tema claro y oscuro.

## Diccionario interactivo

Los términos subrayados con puntos (`<Term id="...">`) muestran su definición y su ejemplo al tocarlos. El texto sale de `docs/GLOSARIO.md` al compilar (`src/lib/glossary.ts`), que sigue siendo la única fuente: la UI no copia definiciones. Un test falla si la UI usa un término que no está en el glosario.

## Dónde vive cada cosa

- `src/components/ui/`: los componentes. Cada uno dice en su encabezado de qué original de 21st.dev o AnimateIcons se inspira y qué se cambió.
- `src/views/`: las vistas, que solo ubican componentes y les pasan datos.
- `src/mocks/`: los datos de ejemplo, con la forma de los contratos (precios como texto decimal, horas en UTC). Los de liquidez (`liquidity.ts`) usan tipos derivados de la spec §5.4.1 hasta que estén los contratos `liquidity_level.v1`, `fvg_event.v1` y `liquidity_stats.v1` (TODO en el archivo). El RSI y el MACD de `us100-1h.json` vienen **precalculados** en el archivo: la UI no los calcula.
- `src/lib/`: formato en español (`format.ts`), el reparto del mapa de mercado (`squarify.ts`, geometría de dibujo) y un hook de ancho.

## Repetidos que se unificaron

- Los dos paneles de vidrio ("glassy dashboard" y "adjustable glass dashboard") → un solo armazón, `glass-shell.tsx`.
- El gráfico de velas y el de indicadores → un solo `trading-chart.tsx`, con paneles.
- Las dos tarjetas holográficas (trofeo del día y de la semana) → un solo `trophy-card.tsx`, con dos acabados.
- Los dos íconos animados comparten un solo hook de animación.
- Una sola librería de animación (`motion`) y un solo control segmentado (`segmented.tsx`).

## Agregar un componente

1. Copiarlo en `src/components/ui/`, anotando el original y la licencia en el encabezado.
2. Si necesita datos, ponerlos en `src/mocks/`, marcados como ejemplo y con la forma del contrato.
3. Usarlo desde una vista de `src/views/`. Nada de lógica de trading en la UI.

## Licencias de terceros

- Íconos animados: AnimateIcons, de Avijit Dey, MIT. Íconos: `lucide-react`, ISC.
- Fuente Manrope (`@fontsource-variable/manrope`), SIL OFL 1.1. Se sirve local, sin CDN.
- `motion`, MIT.

## Comandos

```bash
npm ci          # instalar
npm run dev     # ver la app en el navegador
npm test        # tests
npm run build   # compilar
```
