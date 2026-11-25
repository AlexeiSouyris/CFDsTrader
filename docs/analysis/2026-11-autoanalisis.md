# Autoanálisis del proyecto (25-11-2026)

- **Issue:** #658. **Pedido del dueño:** «Quiero que autoanalices como vamos con el projecto y que si ves cosas que podriamos cambiar».
- **Cómo se hizo:** cinco revisiones en paralelo hechas por agentes de IA (Claude Code), de solo lectura, el 25-11-2026, sobre `main` en `de864d8`:
  1. el repo y el CI (auditoría interna);
  2. los referentes del trading sistemático;
  3. las noticias y sus fuentes;
  4. los proyectos públicos de GitHub;
  5. la UI y otras maneras de operar.
- **Estado:** diagnóstico y propuestas. **Nada de esto cambia reglas ni parámetros:** cada cambio sale en su propio Issue y lo aprueba el dueño. Las ideas de trading son **hipótesis de investigación**, no consejo de inversión.

## Resumen en simple

1. **Construimos rápido y con buen método.**
   - Desde el 18-11: 370 PRs mergeados, unos 2.100 tests y un CI que tarda unos 2 minutos.
   - El método de investigación es exigente: pre-registro congelado, walk-forward, Sharpe deflactado, mitades, costos × 1,5 y placebos.
2. **Ninguna idea pasó todavía.**
   - Son unas 20 hipótesis con veredicto, de 38 filas del registro. Ninguna le gana a comprar y mantener.
   - El motivo principal no es mala suerte, es **estructural**: en velas de 15 minutos sobre CFD, **los costos se comen entre 0,10 y 0,18 R por operación**, y antes de costos la ventaja de nuestras ideas fue ≈ 0.
3. **Gastamos pruebas en ideas de poca probabilidad.** Patrones de gurús, FVG y números redondos, siempre intradía, en los mismos tres índices de EE. UU., que se mueven casi igual, y en el mismo período. Cada programa nuevo abrió un 5 % de error "nuevo". Sumado, el riesgo de un falso positivo en el proyecto ya ronda el 20–25 %.
4. **Lo que más cambiaría el rumbo es pasar a horizontes diarios o semanales y a varios mercados.** Por ejemplo, el seguimiento de tendencia con riesgo ajustado por volatilidad, que es la evidencia publicada más sólida. Ahí el costo pesa menos de 0,02 R por operación, y los datos ya están descargados: 16 futuros diarios desde 2011.
5. **Riesgo:** está bien armado, pero ignora la correlación. Cinco posiciones en US500, US100 y US30 son casi una sola apuesta. Tampoco mide la distribución de caídas con Monte Carlo.

## 1. Cómo lo construimos

| Dato | Valor | Fuente |
|---|---|---|
| PRs mergeados desde el 18-11 | 370 (mediana 448 líneas; el 54 % pasa las 400 de `CLAUDE.md` §2) | `gh pr list` |
| CI | 943 corridas: 85 % bien, 13 % canceladas por re-subidas, 2 % con falla; mediana 2,0 min | `gh run list` |
| Archivo más caliente | `docs/ARCHITECTURE.md`, tocado por el 61 % de los commits; una sola fila llega a 14.897 caracteres | `git log` |

**Fricciones:**
- **El registro de módulos** está armado con filas gigantes de una sola línea, y git no puede mezclar dentro de una línea. Casi cada merge de UI necesitó un rebase a mano.
- **`contracts/README.md`** se edita a mano y choca por lo mismo.
- **La regla de menos de 400 líneas** no se controla.

**Propuestas:**
- **A1.** Un archivo por módulo (`docs/modules/<módulo>.md`), con la tabla generada por un script en el CI. Lo mismo para el índice de contratos.
- **A2.** Un check del CI que avise cuando un PR pasa las 400 líneas sin contar tests ni datos. Y `merge=union` en `.gitattributes` para `registro-de-resultados.csv`, que solo agrega filas.

## 2. Cómo testeamos

**Lo que hay:**
- **Rust:** 1.339 tests, unos 133 de integración con PostgreSQL y Redis en el CI.
- **Python:** 376 tests.
- **UI:** 429 tests.
- **No repintado:** 34 tests.
- **Contratos:** se validan contra el JSON Schema.

**Lo que falta:**
- **B1. Tests de propiedades** (`proptest` en Rust e `hypothesis` en Python) para invariantes como "no mira el futuro", "no repinta" y "el tamaño nunca supera el riesgo". Hoy no hay ninguno.
- **B2. Golden tests contra TradingView:** el ATR dice que es "igual que `ta.atr`", pero no hay un archivo exportado de TradingView que lo pruebe.
- **B3. Pruebas de la UI en un navegador real** (Playwright) con capturas de referencia por vista, en tema oscuro, claro y celular. jsdom no ve las roturas visuales.
- **B4.** Un check que exija que `dsr_trials` coincida con las pruebas del registro, y tests de mutación semanales (cargo-mutants) sobre `risk` y `backtest`.

## 3. Cómo hacemos los backtests

**Lo que está bien:** pre-registro con SHA-256, fuera de muestra intocable desde el 01-11-2025, walk-forward de hasta 51 ventanas, Sharpe deflactado con N = 361, mitades, costos × 1,5, test de suerte con placebos y la comparación contra comprar y mantener pagando costos. Es un nivel alto.

**Lo que cambiaría:**
- **C1. Presupuesto de error global**, por ejemplo un 5 % para todo el proyecto con "alfa-gasto", en vez de un 5 % nuevo por programa.
- **C2. Filtro previo de "borde bruto"**: antes de gastar presupuesto en una idea, estimar con la literatura o con datos descriptivos si su ventaja bruta puede superar 2 veces el costo. Las intradía de 15 minutos casi nunca lo superan.
- **C3. PBO/CSCV** (probabilidad de sobreajuste, Bailey y López de Prado), calculado en Rust junto al Sharpe deflactado.
- **C4. Monte Carlo y bootstrap por bloques** de las caídas y las rachas, y el riesgo de ruina, en cada informe.
- **C5. Spread histórico real**, o al menos escalado con el nivel del índice (#267). Hoy es fijo para 2011–2025 e infla el costo de los años viejos.
- **C6. MAE/MFE sistemático:** cuánto llegó a ganar o perder cada operación antes de cerrar. Hay pistas de que la salida es un problema: la mitad de las perdedoras llegó a +0,4 R.
- **C7. Modelo nulo de Tom Basso:** entrada al azar con nuestras mismas salidas y el mismo tamaño. Si una estrategia no le gana a eso, la entrada no aporta nada. Ya lo usamos en parte con el test de suerte. Habría que hacerlo estándar.

## 4. Sharpe, R y la varianza: cómo vamos

- **R por operación:** lo medimos antes y después de costos en cada estrategia. El problema de fondo es este: **ventaja bruta ≈ 0 y costo de 0,10 a 0,18 R**.
- **Sharpe:**
  - Comprar y mantener: 0,26 mensual (0,24 con la financiación en USD).
  - Ninguna estrategia lo superó.
  - El Sharpe deflactado de nuestras pruebas va de 0,000 a 0,036, contra un umbral de 0,95.
- **"Ganarle a la varianza":** el sistema de riesgo actual tiene:
  - 1 % por operación;
  - pérdida máxima diaria del 3 %;
  - kill switch al 15 % de caída;
  - 5 posiciones como máximo;
  - stop obligatorio de 0,5 a 3 ATR;
  - apalancamiento máximo de 10:1.

  **Le falta:**
  - **D1. Riesgo por grupo correlacionado:** un tope conjunto para los índices de EE. UU. (por ejemplo 1,5 %) y un margen por gaps en el cálculo del tamaño.
  - **D2. Riesgo ajustado por volatilidad a nivel cuenta** (estilo Carver), y bajar el riesgo de a poco cuando hay caída, en vez de todo o nada al 15 %.
  - **D3. Distribución de caídas por Monte Carlo** (es C4), para saber qué caída es "normal" y cuál indica que algo se rompió.

## 5. Otras figuras que operan lo mismo

Las cifras de concursos **no están auditadas**, suelen venir con apalancamiento extremo y casi siempre hay un curso a la venta. Por eso se copia el **proceso**, no la estrategia.

| Referente | Qué aporta | Evidencia |
|---|---|---|
| Moskowitz, Ooi y Pedersen; Hurst (AQR) | Tendencia en muchos mercados, con riesgo por volatilidad | Muy fuerte (papers y un siglo de datos) |
| Robert Carver (ex-AHL, `pysystemtrade`) | Tamaño por volatilidad objetivo, combinar señales y un "buffer" para no operar por cambios chicos | Libros y código abierto; su cuenta no está auditada |
| Gao et al.; Baltussen et al. | Momentum de la última media hora (intradía, una operación por día) | Fuerte en papers, pero gana pocos puntos básicos: con el spread del CFD es dudoso (nuestra R026 no pasó) |
| Kevin Davey | Proceso: walk-forward, Monte Carlo e "incubación" en paper antes de operar | Resultados de concurso sin auditar; vende cursos |
| Tom Basso | Entrada al azar más buenas salidas y buen tamaño, usado como modelo nulo | Anécdota publicada, sin auditar |
| Connors y Raschke | Reversión de corto plazo (RSI(2)) | Un backtest independiente con costos da muy por debajo de comprar y mantener |
| Zarattini y Aziz (ruptura del rango de apertura) | Nuestro caso exacto: US100 intradía | El bruto se replica, pero el neto da ≈ 0 |
| Unger y Williams | Muy poco para nosotros | Sin auditar, con apalancamiento extremo |

## 6. Noticias: otras fuentes y otras maneras

**Huecos de hoy:**
- **Historia con el LLM:** el intérprete solo vale desde su fecha de corte (02-12-2025). Hay apenas 1 año de historia para medirlo.
- **Sorpresa del dato:** sabemos cuándo sale el CPI, pero no cuánto se esperaba.
- **Fuentes de texto:** son solo de bancos centrales.
- **Acierto del intérprete:** 60,8 %, con un intervalo de [47, 73] %, no distinguible del azar.

**Propuestas (cada proveedor nuevo necesita su propio ADR, §6):**
- **N1. Novedad de cada comunicado del FOMC con embeddings, de 2011 a 2025:** cuánto cambió el texto respecto del anterior. No le pide opinión al modelo, así que no arrastra el problema de mirar el futuro. Es la misma fuente oficial del ADR 0016.
- **N2. Empezar ya a guardar `forecast` y `previous` de Forex Factory, más el dato real:** así juntamos la "sorpresa" de acá en adelante. Necesita un ADR que modifique el 0010 §7 y un contrato `economic_event.v2`.
- **N3. Estudio de eventos con la "sorpresa de mercado":** el movimiento del bono a 2 años o del SOFR en los primeros 5 minutos después del CPI, el empleo y el FOMC, con datos que ya tenemos.
- **N4. SEC EDGAR:** resultados (8-K) de las 8 empresas grandes del Nasdaq-100, con la hora exacta al segundo. El User-Agent tiene que llevar un contacto, igual que el BLS, y lo decide el dueño.
- **N5. GDELT:** volumen y tono de las noticias globales desde 2015, gratis y con permiso de guardarlas.

**Descartados:**
- **Kalshi y Polymarket:** están bloqueados en España.
- **Reddit, X, Reuters y AP:** los términos no permiten guardar.
- **VIX9D y VIX3M:** tienen los mismos términos de Cboe, así que los decide el dueño.

## 7. Proyectos públicos de los que tomar ideas

**La licencia define qué se puede tomar:**
- **MIT, Apache, BSD y LGPL** (como librería): se puede tomar código.
- **GPL:** solo ideas.
- **mlfinlab:** es propietario, no se toca.

| Proyecto | Idea concreta | Dónde viviría |
|---|---|---|
| López de Prado y Bailey (papers) | CPCV, PBO, triple barrera y meta-labeling (que el ML decida si tomar la señal, no la dirección) | Rust para el backtest, Python solo para los cortes de ML |
| NautilusTrader (LGPL) y barter-rs (MIT) | El mismo código de estrategia en backtest, paper y real; un registro auditado de cada cambio de estado | Motor en Rust |
| Freqtrade (GPL, solo ideas) | Detección automática de look-ahead y repintado; "protecciones" (pausa después de N pérdidas) | Rust |
| QuantStats y empyrical (Apache) | Vista de Performance tipo *tearsheet*: caídas, mapa de calor mensual, Sortino y Calmar. Las fórmulas de empyrical, como tests de referencia | Métricas en Rust, la UI solo muestra |
| pysystemtrade (GPL, solo ideas) | Tamaño por volatilidad objetivo y buffer de posición | Módulo de riesgo en Rust |
| FinBERT (CC BY-NC-SA) | Clasificador de sentimiento base, local | Python |
| trading-ig (BSD) e ib_async (BSD) | Consultar cómo resuelven el streaming de IG y los errores de cupo | Solo de consulta |

## 8. ¿Otro sistema de UI?

**No hace falta cambiar** React, Vite ni Tailwind: los problemas eran de diseño, y el rediseño por secciones con 21st.dev ya está hecho.

**Mejoras puntuales:**
- **U1. Playwright con capturas de referencia** (es B3). Es lo de más valor.
- **U2. TanStack Table** para las tablas grandes (Performance, Propuestas y operaciones).
- **U3. Gráfico:** mantener el propio. lightweight-charts (Apache 2.0) queda como plan B si aparece un problema de rendimiento.
- **Descartados:**
  - **Advanced Charts de TradingView:** no se da para uso personal.
  - **KLineChart:** calcula indicadores en la UI.
  - **Storybook:** cuesta mucho mantenerlo para un solo constructor.
  - **Tauri:** no hace falta.

## 9. ¿Otra manera de operar?

**Ya probado sobre US500, US100 y US30, sin pasar:**
- tendencia de 12 meses (R016);
- exposición por volatilidad (R019);
- cambio de mes (R015);
- deriva antes del FOMC (R027);
- deriva nocturna (R021 y R022: desapareció desde 2021).

**Lo que no probamos:** diversificar entre clases de activos. Los datos ya están descargados: `config/databento_daily.toml`, con velas diarias desde 2011 de ES, NQ, YM, RTY, FDAX, NKD, ZN, ZB, 6E, 6J, 6B, 6A, GC, SI, HG y CL.

| Orden | Idea | Por qué |
|---|---|---|
| 1 | **Programa 6: tendencia en 16 mercados con riesgo por volatilidad** (EWMAC de 3 velocidades más ruptura), con el momentum simple de 12 meses como control | Es la mejor evidencia publicada, en diario el costo es bajo y los datos ya están. **El criterio se fija antes de correr y no es ganarle al S&P en retorno**: Sharpe de la cartera mayor que 0 con p menor que el umbral, o que mejore una mezcla con comprar y mantener. Además, que se comporte bien en las caídas de la bolsa |
| 2 | **Futuros micro contra CFD como costo**, dentro del mismo backtest | El CFD de IG cobra SOFR + 3 % por año estando largo; el futuro lleva el costo en la base. En un sistema que mantiene posiciones semanas, eso decide. Pasar a dinero real sigue bloqueado (§5) |
| 3 | Reversión diaria (RSI(2) de Connors), la regla publicada sin tocar | Es la única prueba nueva de reversión que vale la pena; las expectativas son bajas |
| — | Pausar ideas intradía de 15 minutos nuevas hasta tener los datos del paper | El costo estructural las condena salvo una ventaja grande |

## 10. Plan propuesto (el dueño elige)

| # | Cambio | Impacto | Esfuerzo | Necesita |
|---|---|---|---|---|
| 1 | Partir `ARCHITECTURE.md` y el índice de contratos (A1), más los checks del flujo (A2) | Alto | Bajo | Nada (es ingeniería) |
| 2 | Presupuesto de error global (C1) y filtro de borde bruto (C2) | Alto | Bajo | OK del dueño (cambia la regla de investigación) |
| 3 | Programa 6: tendencia en 16 mercados (§9) | Alto | Medio | Pre-registro aprobado por el dueño |
| 4 | Riesgo por grupo correlacionado y margen por gaps (D1) | Alto | Medio | OK del dueño (parámetros de riesgo) |
| 5 | Monte Carlo de caídas y riesgo de ruina (C4/D3), más PBO (C3) | Medio-alto | Medio | Nada |
| 6 | Tests de propiedades y golden tests contra TradingView (B1, B2) | Medio | Bajo-medio | Nada |
| 7 | Playwright con capturas (B3/U1) | Medio | Bajo | Nada |
| 8 | Guardar el pronóstico y el dato real del calendario (N2), y la novedad del FOMC (N1) | Medio | Bajo-medio | ADR para N2 |
| 9 | Tamaño por volatilidad y reducción gradual del riesgo por caída (D2) | Medio | Medio | OK del dueño (parámetros de riesgo) |
| 10 | Spread histórico real (C5) y MAE/MFE (C6) | Medio | Medio | Nada |

**Lo que no cambia:** el paper arranca el 27-11 como estaba previsto, porque sus datos son justamente lo que falta para juzgar todo lo demás. El dinero real sigue bloqueado.

## Fuentes principales

- Datos internos: `gh pr list`, `gh run list`, `git log` desde el 18-11, `docs/analysis/registro-de-resultados.csv`, `docs/analysis/diagnostico-us-futuros.md`, `config/research.toml`, `config/risk.toml`.
- [Moskowitz, Ooi y Pedersen 2012](https://doi.org/10.1016/j.jfineco.2011.11.003) · [Hurst, Ooi y Pedersen, AQR](https://www.aqr.com/Insights/Research/Journal-Article/A-Century-of-Evidence-on-Trend-Following-Investing) · [Gao et al. 2018](https://econpapers.repec.org/RePEc:eee:jfinec:v:129:y:2018:i:2:p:394-414) · [Baltussen et al. 2021](https://econpapers.repec.org/RePEc:eee:jfinec:v:142:y:2021:i:1:p:377-403) · [Lucca y Moench (NY Fed SR512)](https://www.newyorkfed.org/research/staff_reports/sr512) · [La deriva nocturna que desaparece (Liberty Street, 2026)](https://libertystreeteconomics.newyorkfed.org/2026/07/the-disappearing-overnight-drift/) · [ORB, Zarattini y Aziz](https://papers.ssrn.com/abstract=4416622) · [Hall of fame de ireallytrade](https://ireallytrade.com/halloffame)
- [GDELT](https://www.gdeltproject.org/about.html) · [SEC EDGAR](https://www.sec.gov/search-filings/edgar-search-assistance/accessing-edgar-data) · [FinBERT](https://huggingface.co/ProsusAI/finbert) · [Bloqueo de Polymarket y Kalshi en España](https://www.cointelegraph.com/news/spain-blocks-polymarket-kalshi-gambling-laws)
- [NautilusTrader](https://github.com/nautechsystems/nautilus_trader) · [barter-rs](https://github.com/barter-rs/barter-rs) · [pysystemtrade](https://github.com/pst-group/pysystemtrade) · [QuantStats](https://github.com/ranaroussi/quantstats) · [Freqtrade](https://github.com/freqtrade/freqtrade) · [lightweight-charts](https://github.com/tradingview/lightweight-charts) · [Preguntas frecuentes de Advanced Charts de TradingView](https://www.tradingview.com/charting-library-docs/v29/getting_started/Frequently-Asked-Questions)
