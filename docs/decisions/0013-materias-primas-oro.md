# 0013 – Materias primas en el catálogo y el motor, empezando por el oro (XAUUSD)

- **Estado:** propuesta
- **Fecha:** 2026-11-24
- **Aprobada por:** pendiente. La aprueba el dueño (modo de un solo constructor, `CLAUDE.md`). Este documento no da nada por aprobado.
- **Issue:** #389 (sigue a #371, opción A)
- **Relacionados:** [ADR 0002](0002-broker-ig.md) (broker IG), [ADR 0011](0011-historico-intradia-databento.md) (histórico de Databento), PR #378 (`GC.v.0` → `XAUUSD` en `config/databento.toml`), spec de la fase 1 §2 y §8

## Contexto

El dueño decidió en #371 (24-11-2026, «dale, hacé la A») bajar el histórico de 1 minuto del oro (`GC.v.0` de Databento, guardado como `XAUUSD`) y dejarlo **guardado sin usar** hasta decidir cinco cosas. Las listó el PR #378:

1. **Clase de activo.** `AssetClass` (Rust, `engine/crates/core/src/instrument.rs`) solo admite `index` y `share`. Lo mismo el `enum` de `contracts/instrument.v1.schema.json` y el `CHECK (asset_class IN ('index', 'share'))` de `infra/migrations/0001_instruments_and_candles.sql`. `InstrumentCosts::from_spec` (`engine/crates/backtest/src/costs.rs`) trata todo lo que no es índice como acción.
2. **Sesión.** El oro opera unas 23 h. `config/sessions.toml` no tiene una sesión para él, y el calendario (`engine/crates/market-data/src/calendar.rs`) rechaza una sesión que cruza la medianoche (`open tiene que ser anterior a close`).
3. **Financiación.** IG usa para las materias primas una fórmula distinta de la de índices, y el motor no la implementa.
4. **Moneda.** El contrato de oro de IG es en USD. La cuenta es en EUR, y hoy el backtest rechaza un instrumento con otra moneda (`SetupError::CurrencyConversion`, `engine/crates/backtest/src/setup.rs`).
5. **Apalancamiento.** La spec §8 fija topes para índices (10:1) y acciones (3:1), no para materias primas.

### Lo que se comprobó (24-11-2026)

| Punto | Dato | Fuente |
|---|---|---|
| Contrato de oro de IG | «Oro al contado», valor de un contrato **100 $**, spread mínimo **0,9**. Ninguna materia prima de la tabla de CFD está en EUR, y no hay contrato mini de oro | [ig.com/es/nuestros-costes](https://www.ig.com/es/nuestros-costes), tabla de materias primas (CFD) |
| Margen de IG para el oro | Clientes minoristas: **5 %** (1:20). Plata, cobre y crudo: 10 % (1:10) | [ig.com/es/margenes](https://www.ig.com/es/margenes), tabla «Materias primas (CFD)» |
| Financiación de IG para materias primas al contado | «Fórmula del ajuste del coste de financiación nocturna para materias primas»: tamaño de operación × (base ± costes de IG), por noche. «Fórmula de la base = (P3 – P2) / (T2 – T1)», con P2 y P3 los precios del futuro anterior y siguiente, y T1 y T2 sus vencimientos. «Fórmula para el coste de IG = Precio medio sin vencimiento x 3 % / 365». Si se paga la base, se suma el coste de IG; si se recibe, se resta | [ig.com/es/nuestros-costes](https://www.ig.com/es/nuestros-costes) |
| Hora del cargo | Posición abierta pasadas las 23:00 CE(S)T (la misma que ya usa el catálogo, `funding_cutoff`) | Ídem |
| Margen regulatorio en España | Garantía inicial del **5 %** del nocional cuando el subyacente es el oro (máximo 20:1). **10 %** para las materias primas distintas del oro (máximo 10:1) | CNMV, [Resolución de 27 de junio de 2019](https://www.boe.es/eli/es/res/2019/06/27/(1)) (BOE-A-2019-9737), anexo I, que aplica en España las medidas de la [Decisión (UE) 2018/796 de ESMA](https://eur-lex.europa.eu/legal-content/ES/TXT/?uri=CELEX:32018X0601(02)), anexo I |
| Horario del futuro de oro (GC) | CME Globex, domingo a viernes de 18:00 a 17:00 hora de Nueva York, con una pausa diaria de 60 minutos desde las 17:00 | CME Group, [fact card de futuros y opciones de oro](https://www.cmegroup.com/content/dam/cmegroup/market-regulation/files/gold-futures-and-options-fact-card.pdf) |
| Tipo de cambio EUR/USD oficial | Tipos de referencia del BCE: «usually updated at around 16:00 CET every working day», y «published for information purposes only» | [BCE, euro foreign exchange reference rates](https://www.ecb.europa.eu/stats/policy_and_exchange_rates/euro_reference_exchange_rates/html/index.en.html) |

### Lo que no se pudo confirmar (queda pendiente, no se inventa)

| Dato | Por qué falta | Cómo confirmarlo |
|---|---|---|
| Epic, valor por punto, tamaño mínimo y tick del oro al contado | La web no los publica. El PR #378 menciona un mínimo de 0,05 contratos sin fuente pública | API de IG, `GET /markets/{epic}`, cuando haya cupo (#360) |
| Horario de negociación del oro en IG | La página de costes solo dice que «las horas de negociación varían según el mercado» | Ficha del mercado en la API (`openingHours`) o la plataforma |
| Qué futuros usa IG como P2 y P3 de la base, y cuándo pasa al siguiente | La página da la fórmula, no los contratos | Preguntar a IG o medir en la demo |
| Cómo cuenta las noches del fin de semana para materias primas | La página no lo dice | Medir en la demo |
| Comisión o margen de IG al convertir USD a EUR | No se buscó una cifra en la web de IG | Página de costes de IG o la demo |
| Spread típico del oro (no el mínimo) | Solo está el mínimo publicado | Medir en la demo como en #67 |
| Feriados de CME para el oro | No hay archivo `holidays/` para CME | Calendario oficial de CME |

## Decisión (propuesta)

Cada punto tiene opciones con pros y contras en «Alternativas consideradas». Esto es lo que se recomienda:

1. **Clase de activo `commodity`** como cambio **compatible** de `instrument.v1`: se agrega `"commodity"` al `enum` de `asset_class`. Ningún consumidor de Python ni de la UI lee `asset_class` hoy (`git grep` en `intelligence/` y `ui/src`), y un documento viejo sigue siendo válido. En el mismo PR: `AssetClass::Commodity` en Rust (con `as_str` y `FromStr`), una **migración nueva** que reemplaza el `CHECK` de la 0001 por `('index', 'share', 'commodity')`, e `InstrumentCosts::from_spec` con una rama propia para `commodity` (sin comisión, spread y financiación de materia prima), en vez del `_ =>` que hoy la trataría como acción.
2. **Sesión del oro en dos capas:**
   - **Horario de mercado** (velas, validación de datos, cuándo un stop puede ejecutarse): una sesión nueva en `config/sessions.toml`, por ejemplo `[sessions.CMEGLOBEX_METALS]`, de domingo 18:00 a viernes 17:00 `America/New_York` con la pausa diaria de 17:00 a 18:00. El calendario tiene que aceptar una sesión que cruza la medianoche (hoy no lo hace) y un archivo de feriados de CME. Hasta confirmar el horario de IG, se usa el de CME como aproximación, marcada como tal.
   - **Ventana de entradas:** como en la spec §2, solo se abren operaciones en horas en que alguien puede aprobarlas. Hoy `config/trading_window.toml` solo recorta minutos al principio y al final de la sesión; hay que sumarle una ventana horaria propia por instrumento o grupo (horas en `America/New_York`). Las horas exactas las elige el dueño; no se proponen acá.
   - El cierre de los viernes (spec §8) es la última vela de 15 m antes de las 17:00 de Nueva York.
3. **Financiación con la fórmula de IG para materias primas**, como una variante nueva del costo de financiación en Rust (`engine/crates/backtest/src/costs.rs`), con parámetros en `config/instruments.toml` (`funding_model = "commodity_basis"`, `funding_admin_fee_pct = 3`, `funding_day_count = 365`, todos con fuente `ig-es-costes`). La base sale de la curva de futuros de Databento (dos vencimientos de GC), que ya se baja para el oro, con la hora en que cada precio estuvo disponible (sin look-ahead). Mientras no se confirme qué contratos usa IG, la base queda `pending` y el backtest validado se frena (spec §11). El estudio puede correr con una base **supuesta y marcada como tal en el informe**, como se hizo con US2000 en #371.
4. **Contrato en USD, cuenta en EUR, con conversión explícita:**
   - El oro se carga con `currency = "USD"` en el catálogo, que es lo que ofrece IG.
   - El motor convierte P/L, margen, financiación y riesgo con el **tipo EUR/USD vigente en cada momento**. La spec §8 ya lo prevé: «€ por punto, convertido a la moneda del instrumento».
   - Fuente propuesta para el backtest: el tipo de referencia diario del BCE (serie `EXR.D.USD.EUR.SP00.A`), por la misma API que ya se usa para el €STR (`config/reference_rates.toml`). Vale desde su publicación (16:00 CET), nunca antes. El BCE dice que es solo informativo: el error contra el tipo real de IG se acepta en el backtest y se mide en paper.
   - La comisión de conversión de IG queda `pending` hasta confirmarla. Mientras falte, el backtest validado se frena.
   - `SetupError::CurrencyConversion` deja de ser un error y pasa a pedir la serie de tipo de cambio. El campo `account_to_instrument` de `MarketTerms` deja de ser constante.
5. **Tope de apalancamiento de 10:1 para el oro** y de **5:1 para las demás materias primas**, como topes propios por debajo del regulatorio (20:1 y 10:1, CNMV y ESMA). Es la mitad del máximo regulatorio, el mismo criterio que índices (10:1 contra 20:1). Cambia la spec §8 («índices 10:1, acciones 3:1») y, por la spec §1.2, es un cambio de parámetros de riesgo: lo decide el dueño.

**Orden de implementación (Issues aparte, uno por tema):** (1) clase de activo y migración; (2) calendario con sesiones de 23 h; (3) conversión de moneda; (4) financiación de materias primas; (5) oro en el catálogo, con todos los pendientes; (6) estudio en backtest y walk-forward. Hasta el punto 5, el oro sigue fuera del catálogo y nadie lo opera, ni en paper.

## Alternativas consideradas

### 1. Clase de activo

| Opción | Pros | Contras |
|---|---|---|
| **A. `commodity` en `instrument.v1` (recomendada)** | Cambio chico y compatible. Sirve para plata, crudo y cobre más adelante | Toca Rust, el contrato y una migración en el mismo PR |
| B. `instrument.v2` con una clase nueva | Separa versiones si después se quiere cambiar más cosas | Hay que migrar a todos los consumidores de `instrument.v1` (17 contratos lo referencian). Es demasiado para agregar un valor al `enum` |
| C. Cargar el oro como `index` | No cambia nada | Es falso. El costo, el margen regulatorio y la financiación del oro no son los de un índice, y mezclarlos esconde errores |

### 2. Sesión

| Opción | Pros | Contras |
|---|---|---|
| **A. Horario de mercado de 23 h + ventana de entradas propia (recomendada)** | Velas y stops reflejan el mercado real. Las entradas siguen la regla de la spec §2 (alguien tiene que poder aprobarlas) | Hay que extender el calendario para sesiones que cruzan la medianoche y sumar los feriados de CME |
| B. Tratar el oro con el horario de la bolsa de Nueva York (09:30–16:00) | No cambia el calendario | Ignora unas 16 h de mercado. Un stop o un gap nocturno quedarían fuera del backtest, que se vería mejor de lo que es |
| C. Sesión continua de 24 × 5 sin pausa | Simple | No refleja la pausa diaria de CME y marca como faltantes velas que nunca existieron |

### 3. Financiación

| Opción | Pros | Contras |
|---|---|---|
| **A. Fórmula de IG para materias primas, con la base sacada de la curva de futuros (recomendada)** | Es la fórmula publicada por IG. Los datos de los futuros ya se bajan de Databento | Hay que saber qué vencimientos usa IG (pendiente) y bajar dos contratos, no solo el continuo `v.0` |
| B. Reusar la fórmula de índices con un tipo de USD (por ejemplo SOFR) ± 3 % | Ya está implementada | No es la fórmula de IG para materias primas. Violaría «no inventar» (CLAUDE.md §5b) |
| C. Costo fijo supuesto | Permite correr el estudio ya | Solo sirve para un estudio marcado como tal. No vale para validar un backtest |

### 4. Moneda

| Opción | Pros | Contras |
|---|---|---|
| **A. Contrato en USD, conversión con el tipo de referencia diario del BCE (recomendada)** | Fuente oficial, gratis y con hora de publicación conocida (sin look-ahead). Usa la misma API que el €STR | Es diario e informativo: no es el tipo de IG ni el intradía. Error chico en P/L y margen, que se mide en paper |
| B. Conversión con EUR/USD intradía de IG | Es el tipo más parecido al que aplica IG | Gasta cupo de la API de IG (#360) y no hay histórico largo |
| C. Conversión con el futuro de euro de CME (6E) de Databento | Intradía, histórico largo | Cuesta créditos de Databento y es un futuro, no el tipo al contado |
| D. Buscar un contrato de oro en EUR | No hace falta convertir | La tabla pública de IG no tiene ninguno (24-11-2026) |

### 5. Tope de apalancamiento

| Opción | Pros | Contras |
|---|---|---|
| **A. Oro 10:1, otras materias primas 5:1 (recomendada)** | Mismo criterio que índices: la mitad del máximo regulatorio | Con 1.000 € de capital y el contrato de 100 $, puede que muchas operaciones queden debajo del mínimo del broker y no se abran (spec §8). El estudio lo va a medir |
| B. Oro 5:1 | Más conservador | Con el capital actual, todavía menos operaciones posibles |
| C. El máximo regulatorio (20:1) | Más operaciones posibles | Contradice la idea de la spec de quedar por debajo de ESMA |

## Consecuencias

- **Lo que se habilita:** el oro entra al catálogo y al estudio con las mismas reglas que los índices (costos con fuente, pendientes que frenan el backtest validado, ventana de entradas). Plata, crudo y cobre quedan a un paso, con su tope de 5:1.
- **Lo que se vuelve más difícil:**
  - El calendario tiene que manejar sesiones que cruzan la medianoche y una pausa diaria. Afecta las velas esperadas, la validación de datos y los cierres de los viernes.
  - El motor pasa a manejar dos monedas. Riesgo, margen, P/L y financiación tienen que convertir siempre con el tipo vigente en ese momento, y cada conversión necesita tests (sin tipo publicado → error, nunca un tipo inventado).
  - Hay una segunda fórmula de financiación, con datos de la curva de futuros.
- **Tamaño mínimo:** con 1.000 € de capital y riesgo del 1 % (10 €), un contrato de 100 $ por punto puede exigir más riesgo del permitido incluso con el tamaño mínimo, que todavía no está confirmado. La regla de la spec §8 ya cubre ese caso: si el tamaño es menor al mínimo del broker, no se abre.
- **Nada de esto habilita dinero real** ni operar el oro en paper. Operarlo en paper necesita, después de este ADR, el catálogo completo (sin pendientes) y el estudio de #371.
- **Pendientes que siguen abiertos aunque se apruebe:** los de la tabla «Lo que no se pudo confirmar». Ninguno se inventa: cada uno queda `pending` en `config/instruments.toml` con su motivo.
