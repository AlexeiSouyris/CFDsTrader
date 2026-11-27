# Golden tests contra TradingView: cómo exportar los indicadores

El motor dice que su ATR es «igual que `ta.atr`» y su EMA «igual que `ta.ema`» de TradingView. Hoy eso se prueba de dos formas que **no necesitan a nadie** (#666):

- `engine/crates/indicators/tests/reference.rs` compara SMA, EMA y ATR con las fórmulas publicadas de Pine Script, en aritmética exacta, y con TA-Lib. Las velas son **sintéticas**: el fixture `tests/fixtures/reference_indicators.csv` lo genera `tools/verify/make_indicator_fixture.py`.
- `tools/verify/compare_reference.py` compara contra TA-Lib sobre velas reales guardadas (no se commitean).

Falta la prueba de verdad: los números que muestra **TradingView** para las mismas velas. Eso solo lo puede hacer el dueño, con su cuenta. Esta guía explica cómo, paso a paso. Toma unos 10 minutos.

> **Importante:** la exportación tiene precios de un proveedor de datos. El repositorio es público: **el CSV no se commitea nunca**. Se guarda fuera del repo y el test lo lee desde una variable de entorno.

## Qué se compara

| Indicador | En el motor | En TradingView | Comparación |
|---|---|---|---|
| SMA(50) | `cfds_indicators::sma` | `ta.sma(close, 50)` | desde su primer valor |
| EMA(10) y EMA(365) | `cfds_indicators::ema` | `ta.ema(close, n)` | desde la vela 400 |
| ATR(14) de Wilder | `cfds_indicators::wilder_atr` | `ta.atr(14)` | desde la vela 400 |
| RSI(14) | **no existe en el motor** | `ta.rsi(close, 14)` | se exporta igual, para cuando se agregue |

EMA y ATR se comparan desde la vela 400 porque TradingView los siembra en la primera vela de **su** historia, y la exportación puede empezar más tarde. Pasadas unas cientos de velas, el suavizado borra esa diferencia. La SMA no tiene memoria: coincide desde el principio.

No importa de qué proveedor sean las velas de TradingView (no tienen que ser las de IG): el test recalcula los indicadores del motor **sobre las mismas velas de la exportación** y compara.

## Paso 1: abrir el gráfico

1. Entrá a TradingView con tu cuenta.
2. Abrí un gráfico de **un solo instrumento y un solo timeframe**. Sugerencia: el S&P 500 en **1 hora** (por ejemplo, el CFD `US500` de cualquier broker de la lista, o el índice `SPX`).
3. Cargá bastante historia: con la vista en 1 hora, desplazate hacia la izquierda hasta tener **al menos 1000 velas** (unos dos meses). Cuantas más, mejor: las primeras 400 no se comparan para EMA y ATR.

## Paso 2: agregar el script de los indicadores

Así las columnas de la exportación tienen siempre los mismos nombres.

1. Abajo, abrí el **Editor de Pine**.
2. Creá un indicador nuevo, borrá lo que trae y pegá esto:

```pine
//@version=5
indicator("CFDsTrader golden", overlay = true)
plot(ta.sma(close, 50), title = "sma50")
plot(ta.ema(close, 10), title = "ema10")
plot(ta.ema(close, 365), title = "ema365")
plot(ta.atr(14), title = "atr14")
plot(ta.rsi(close, 14), title = "rsi14")
```

3. Guardalo y tocá **Agregar al gráfico**. No hace falta que se vea bien: el RSI y el ATR quedan en la escala del precio, no importa.

## Paso 3: exportar

1. Arriba, en el menú del diseño del gráfico (la flecha al lado del nombre del diseño), elegí **Exportar datos del gráfico…** (*Export chart data…*).
2. Formato de la hora: **ISO** o **UNIX**, da igual (el test no la usa).
3. Descargá el CSV y guardalo **fuera del repo**, por ejemplo en `~/cfds-golden/us500_1h.csv`.

Si la opción de exportar no aparece, puede depender del plan de TradingView. En ese caso, avisá en el Issue #666 y lo vemos.

## Paso 4: revisar las columnas

Abrí el CSV con un editor de texto y fijate en la primera línea. Tiene que tener `open`, `high`, `low` y `close`, y columnas llamadas `sma50`, `ema10`, `ema365`, `atr14` y `rsi14`. Si TradingView les puso otro nombre (por ejemplo, con el nombre del indicador adelante), **no cambies el archivo a mano**: pasá la primera línea en el Issue #666 y se ajusta el test.

## Paso 5: correr el golden test (en el Mac mini)

```bash
cd engine
CFDS_TV_GOLDEN=~/cfds-golden/us500_1h.csv \
  cargo test -p cfds-indicators --test reference -- --ignored
```

- **Pasa:** el motor coincide con TradingView en esas velas (tolerancia relativa de 1e-6, que es lo que TradingView guarda en `float`). Anotalo en el Issue #666: instrumento, timeframe, cantidad de velas y fecha.
- **Falla:** el mensaje dice qué indicador, en qué vela y los dos valores. Pegalo en el Issue. No es para corregir a mano: se abre un Issue con el caso.

## Resumen

| Qué | Dónde | Corre en el CI |
|---|---|---|
| Fórmulas de Pine en aritmética exacta y TA-Lib, sobre velas sintéticas | `engine/crates/indicators/tests/reference.rs` | sí |
| TA-Lib sobre velas reales guardadas | `tools/verify/compare_reference.py` | no (las velas no se publican) |
| Exportación real de TradingView | el mismo `reference.rs`, test `--ignored` | no (la exportación no se publica) |
