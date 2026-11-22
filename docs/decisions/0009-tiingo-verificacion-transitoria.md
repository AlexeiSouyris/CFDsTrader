# 0009 – Tiingo (plan gratis) solo para verificación transitoria

- **Estado:** propuesta
- **Fecha:** 2026-11-23
- **Aprobada por:** pendiente. La aprueba el dueño, por el modo de un solo constructor de `CLAUDE.md`. No habilita dinero real.
- **Issue:** #203 (relacionado: ADR 0005, `docs/research/precios-componentes-nasdaq100-2026-11.md`, #132, #140, #167)

## Contexto

El dueño sacó un token de Tiingo, plan gratis "Starter". Ya está en el `.env` del Mac mini como `TIINGO_API_KEY`. Quiere usarlo **sin que se repita lo del cupo de IG**: allá se gastó cupo sin saber quién ni cuánto, y hubo que agregar después un registro de uso (#140) y un presupuesto por uso con kill switch (#167).

Lo que dicen los términos y la página de precios de Tiingo (leídos el 23-11-2026 en [tiingo.com/tos](https://www.tiingo.com/tos) y [tiingo.com/about/pricing](https://www.tiingo.com/about/pricing)):

| Tema | Plan Starter (gratis) |
|---|---|
| Guardar datos | **Prohibido** (§1.6(a)): "you may not write, save, archive, back up, or otherwise retain Tiingo Data in any persistent or durable storage". Incluye base de datos, archivos, caché, logs y colas. Solo se pueden procesar **en memoria y de forma transitoria** |
| Productos derivados | §1.6(c): los productos derivados de los datos de Tiingo tampoco se pueden guardar |
| Licencia | Uso interno |
| Límites | 50 pedidos por hora, 1.000 por día, 500 símbolos por mes y 1 GB por mes. El panel del dueño dice 2 GB: se toma el menor. Se reinician cada hora, a la medianoche "EST" y cada mes |
| Cobertura | Acciones, ETFs, FX y cripto de EE. UU. **No tiene índices**: ni DAX, ni NDX, ni SPX, ni Dow |
| Intradía | Velas del feed de IEX, que no publica su profundidad. El remuestreo a 15 min **no está verificado**: la documentación solo muestra 5 min |

Lo que ya sabemos de antes:
- **No sirve como histórico.** El ADR 0005 y la investigación de noviembre ya descartaron Tiingo gratis por la prohibición de guardar.
- **Nuestros datos son CFD de índices de IG** (US500, US100, US30 y DE40). Tiingo no tiene esos índices, pero sí ETFs que los siguen: SPY (S&P 500), QQQ (NASDAQ 100) y DIA (Dow Jones). No hay un ETF de EE. UU. equivalente al DAX en moneda local.

## Decisión (propuesta)

### 1. Uso permitido: verificación transitoria, nunca histórico

Tiingo se usa **solo para controlar nuestras velas**, comparándolas en memoria con un ETF que sigue el mismo índice:

| Nuestro instrumento | ETF de Tiingo |
|---|---|
| US500 | SPY |
| US100 | QQQ |
| US30 | DIA |
| DE40 | ninguno: no se verifica |

**Qué se compara.** El nivel de un ETF no es el del índice: SPY es una fracción del S&P 500, paga dividendos y tiene comisión. Por eso no se comparan precios, sino **variaciones de un día al otro**, a la misma hora:
- **Preferido:** el cierre de nuestra vela de 15m que termina a las 16:00 de Nueva York contra el cierre diario del ETF, que es a esa hora. Es la comparación justa.
- **Si no hay velas de 15m** de esa fecha: nuestra vela diaria (#103, de medianoche a medianoche hora de Nueva York), con una tolerancia más ancha, porque el CFD sigue cotizando después de las 16:00.
- **Días ex-dividendo:** el ETF baja por el dividendo y el índice no. Si la respuesta de Tiingo trae el dividendo del día, se suma antes de comparar. Si no lo trae (a verificar en la documentación en el PR de implementación), esos días se marcan aparte y no cuentan como diferencia.
- **Tolerancias** en `config/`, sin números inventados. Se calibran en el PR de implementación mirando la distribución de diferencias en memoria, y el dueño las aprueba ahí.

**Qué detecta.** Errores de nuestros datos: un día que falta, una vela corrida de hora, un cierre que no corresponde o un salto que el ETF no tuvo.

**Qué no hace nunca:**
- Guardar precios, velas ni respuestas de Tiingo en ningún lado: base, archivos, caché, logs, colas o Redis.
- Alimentar la estrategia, el backtest, los análogos ni el ML. La verificación no cambia ninguna vela: solo avisa.
- Rellenar huecos con datos de Tiingo.

### 2. Qué se puede guardar del resultado: pregunta abierta para el dueño (§1.6(c))

La comparación da, por fecha e instrumento, algo como "coincide o no" y "diferencia de X %". Ese resultado se calcula **con** datos de Tiingo, así que podría contar como "Derived Product" según §1.6(c). No está claro:
- **A favor de que no lo sea:** no se puede reconstruir ningún precio de Tiingo a partir de un "sí/no", y es un control de calidad de **nuestros** datos.
- **En contra:** la diferencia en % se calcula directamente con la variación del ETF. Junto con nuestra variación, que sí guardamos, permite recalcular la de Tiingo. Eso se parece mucho a guardar un dato derivado.

Opciones, de la más segura a la menos:

| Opción | Qué queda guardado | Riesgo con los términos |
|---|---|---|
| **A** | **Nada.** El resultado se muestra en la terminal y se pierde. Solo se corre a mano; nunca desde `cron`, porque `cron` lo escribiría en un log | Ninguno |
| **B** | Solo el veredicto por fecha e instrumento ("coincide", "no coincide" o "sin dato"), **sin** la diferencia ni ningún número de Tiingo | Bajo, pero dudoso |
| **C** | El veredicto **y** la diferencia en % | Alto: con nuestra variación se reconstruye la del ETF |

**Recomendación:** empezar con **A**. Pasar a **B** solo si el dueño lo aprueba, idealmente después de preguntarle a Tiingo por escrito si un veredicto de control de calidad cuenta como producto derivado. **C** queda descartada mientras el plan sea el gratis.

**Pregunta al dueño:** ¿A, o B con la consulta a Tiingo?

### 3. Presupuesto, registro de uso y kill switch (lo que aprendimos con IG)

Todo en un archivo nuevo, `config/tiingo.toml`, validado al arrancar. Valores propuestos, con margen sobre los límites del plan:

| Límite del plan | Tope propio | Ventana con la que se cuenta |
|---|---|---|
| 50 pedidos por hora | **40** | últimos 60 minutos |
| 1.000 pedidos por día | **800** | últimas 24 horas |
| 500 símbolos por mes | **20** (hacen falta 3) | últimos 31 días |
| 1 GB por mes (el panel dice 2 GB) | **800 MB** | últimos 31 días |

- **Ventanas móviles.** Tiingo reinicia a la medianoche "EST" y no está claro si eso cambia con el horario de verano. Una ventana móvil nunca cuenta menos que una de calendario, así que respeta el límite con cualquier interpretación del reinicio.
- **Registro de uso**: una tabla de solo agregar, como `ig_allowance_log` (#140). Se crea con una migración en el PR de implementación, no en este. Por cada pedido guarda:
  - fecha y hora en UTC;
  - tipo de pedido (por ejemplo `eod`);
  - el **símbolo pedido** (el nuestro, no un dato de Tiingo), para contar símbolos;
  - el código HTTP;
  - los bytes recibidos.

  **Nunca precios, nunca el cuerpo de la respuesta.**
- **Antes de cada pedido** se suma lo del registro. Si el pedido no entra en algún tope, no se hace: kill switch.
- **Kill switch:** se corta la corrida, sin reintentar en bucle, ante:
  - un tope propio alcanzado;
  - una respuesta 429 o de autenticación;
  - una respuesta con una forma inesperada;
  - un error de red repetido.

  El mensaje dice qué tope y cuánto se usó, sin datos de Tiingo.
- **Una sola máquina.** El registro vive en la base del Mac mini, así que solo ahí se puede contar el uso. Como con IG (#143), Tiingo se usa **solo con `CFDS_DATA_MACHINE=1`**. En cualquier otra máquina, como las sesiones en la nube, el comando se niega antes de pedir nada.
- **Token:** `TIINGO_API_KEY` sale del `.env`. Nunca se imprime ni se guarda en logs. En `.env.example` va con un valor falso.
- **Nada de caché en disco**: el cliente HTTP no puede escribir respuestas en disco. Los errores se registran sin el cuerpo de la respuesta.

### 4. Dónde vive

- **Rust, en un crate propio `engine/crates/tiingo`**, como `ig`: la validación de datos de mercado es del motor (CLAUDE.md §1), y los proveedores reales van en su propio crate (registro de módulos, fila `market-data`).
- **Un comando que se corre a mano** (opción A) y muestra el resultado en la terminal.

### 5. Orden

1. El dueño aprueba este ADR y elige A o B.
2. PR de implementación: crate, `config/tiingo.toml`, migración del registro de uso y tests sin red (respuestas armadas a mano en los tests, nunca grabadas de Tiingo).
3. Una primera corrida a mano con un rango corto, por ejemplo 10 días: unos pocos pedidos.

## Alternativas consideradas

- **Plan pago (30 USD por mes para individuos).** Permite guardar mientras esté activo, pero al cancelarlo hay que **borrar todo** (§1.6(b)): los datos nunca quedan nuestros. Además cuesta, y el enfoque de v1 es sin costo (ADR 0004 §7). Descartado.
- **Usarlo como fuente de histórico o para rellenar huecos.** Prohibido en el plan gratis (§1.6(a)) y ya descartado en el ADR 0005. Además no tiene los índices, solo ETFs. Descartado.
- **Usar velas intradía de IEX para comparar velas de 15m.** IEX no publica su profundidad, y el remuestreo a 15 min no está verificado. Gastaría muchos más pedidos y bytes. Queda fuera de v1. Si hiciera falta, se verifica primero en la documentación y vuelve como cambio a este ADR.
- **Verificar con IG mismo** (pedir dos veces la misma vela). Gasta cupo de IG, que es justo lo que hay que cuidar, y no detecta un error del propio proveedor. Descartado.
- **No verificar nada.** Es lo de hoy. La validación interna de las velas (#5, #89) detecta datos imposibles, pero no un cierre plausible y equivocado ni una vela corrida de hora. Tiingo agrega un control externo gratis.
- **Opción C del punto 2 (guardar la diferencia en %).** Descartada mientras el plan sea el gratis: con nuestra variación permite reconstruir la del ETF.

## Consecuencias

- **Más fácil:** hay un control externo y gratis de nuestras velas de US500, US100 y US30, con el uso medido desde el primer pedido y topes que no dejan pasar los límites del plan.
- **Más difícil:**
  - Con la opción A no queda historia de las verificaciones: cada control es un momento, y hay que volver a correrlo.
  - DE40 no tiene verificación.
  - Las tolerancias hay que calibrarlas, y no se puede guardar lo que se usó para calibrarlas.
- **Riesgo con los términos:** se cuida el alcance de §1.6(a) y §1.6(c). Si Tiingo cambia sus términos o sus límites, este ADR se revisa antes de seguir usándolo.
- **No cambia** la estrategia, el backtest, los contratos ni el modo simulación (paper).
