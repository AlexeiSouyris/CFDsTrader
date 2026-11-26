# 5.1 Mapa de liquidez

> **Nivel 5 · Liquidez.** Educativo: no es consejo de inversión ni promete ganancias. Todo pasa en
> simulación ([paper trading](../GLOSARIO.md#paper-trading)). Los precios de los ejemplos son
> redondos, no cotizaciones.

## Objetivo

Conocer los niveles del mapa de [liquidez](../GLOSARIO.md#liquidez) del motor (máximos y mínimos
iguales, del día y de la semana anteriores, de cada sesión y números redondos) y su estado:
**intacto**, **barrido** o **roto**.

## La idea

Un **nivel de liquidez** es un precio donde **probablemente** hay órdenes esperando (stops arriba
de un máximo, stops abajo de un mínimo). No vemos esas órdenes: el mapa marca precios donde la
gente suele ponerlas, con una regla medible para cada uno (`config/liquidity.toml`, spec §5.4.1):

| Nivel | Regla del motor | Existe desde |
|---|---|---|
| **EQH / EQL** (máximos o mínimos iguales) | Al menos **2 pivotes** de 15 m de las últimas **200 velas** cuya diferencia es como mucho **0,10 × ATR** de 15 m. El nivel es el más alto de esos máximos (el más bajo, en EQL). | La vela que confirma el último pivote. |
| **PDH / PDL** (día anterior) | Máximo y mínimo de la sesión anterior del instrumento. | El cierre de esa sesión. |
| **PWH / PWL** (semana anterior) | Máximo y mínimo de las sesiones de la semana anterior (lunes a viernes). | El cierre de la última sesión de esa semana. |
| **Sesiones** | Máximo y mínimo de cada ventana UTC: **Asia 00:00–07:00, Londres 07:00–13:30, Nueva York 13:30–20:00**. | El cierre de la última vela de la ventana. |
| **Números redondos** | El múltiplo del paso más cercano **arriba** y **abajo** del cierre: US500 cada 50 y 100, US100 cada 100, US30 cada 500, DE40 cada 100. | Cada vela. |

**Los estados de un nivel alto** (el bajo es el espejo), mirando vela por vela de 15 m:

- **intacto**: ninguna vela cerrada tuvo un máximo arriba del nivel;
- **barrido**: la **primera** vela que pasa el nivel **cierra en el nivel o abajo** (pinchó y
  volvió): es el sweep de la lección 5.2;
- **roto**: la primera vela que pasa el nivel **cierra arriba**.

Un nivel deja de estar activo cuando se barre, se rompe o pasan **500 velas**. El estado se guarda
con la vela que lo cambió y no se reescribe nunca.

**Importante:** el mapa es **informativo**. No suma al puntaje de la estrategia hasta validarse con
su propio criterio. "Ahí hay stops" es una hipótesis razonable, no un dato: no tenemos el libro de
órdenes.

> **Probá antes de seguir:** el US500 cierra en 6.020. ¿Qué números redondos marca el mapa?
>
> <details><summary>Ver la respuesta</summary>
>
> Con paso 50: 6.050 arriba y 6.000 abajo. Con paso 100: 6.100 arriba y 6.000 abajo.
>
> </details>

## Interactivo

La capa del mapa de liquidez sobre un día real: cada nivel con su nombre, su estado (intacto,
barrido, roto o vencido) y la vela en que cambió.

## Ejemplo resuelto

Dos pivotes altos de 15 m en 6.050,0 y 6.051,5, dentro de las últimas 200 velas, con un ATR de 20
puntos.

- Tolerancia: 0,10 × 20 = **2 puntos**. La diferencia es 1,5: **EQH en 6.051,5** (el más alto).
- Más tarde, la primera vela que pasa 6.051,5 tiene máximo 6.055 y cierra en 6.049: **barrido**.
- Si hubiera cerrado en 6.054, el nivel quedaba **roto**.

## Ejercicio

En un día real del replay de práctica, mirá el mapa a media mañana (sin avanzar) y apostá cuál de
los niveles intactos se barre primero. Avanzá y comprobá. Sin nota.

<details><summary>Ver la solución</summary>

No hay solución fija: el objetivo es ver que "el nivel más cercano" no siempre es el primero en
caer, y que muchos niveles terminan **rotos** y no barridos. Anotá tu acierto o error en el Diario:
con muchos días, vas a ver tu tasa real.

</details>

## Examen

6 preguntas. Para aprobar hace falta el 80 % (umbral provisorio).

## Desbloquea

- **Mapa de liquidez** del motor sobre el gráfico.

## Cifras y fuentes

| Cifra | Valor | Fuente | Prueba |
|---|---|---|---|
| EQH/EQL: pivotes mínimos | 2 (`2`) | `config/liquidity.toml` `equal.min_touches` | A |
| EQH/EQL: ventana | 200 velas (`200`) | `config/liquidity.toml` `equal.lookback_bars` | A |
| EQH/EQL: diferencia máxima | 0,10 ATR (`0.10`) | `config/liquidity.toml` `equal.tolerance_atr` | A |
| Sesión de Asia (UTC) | desde las `00:00` | `config/liquidity.toml` `sessions_utc.asia.start` | A |
| Sesión de Londres (UTC) | desde las `07:00` | `config/liquidity.toml` `sessions_utc.london.start` | A |
| Sesión de Nueva York (UTC) | desde las `13:30` | `config/liquidity.toml` `sessions_utc.ny.start` | A |
| Fin de la sesión de Nueva York (UTC) | `20:00` | `config/liquidity.toml` `sessions_utc.ny.end` | A |
| Pasos de los números redondos | `US500 = ["50", "100"], US100 = ["100"], US30 = ["500"], DE40 = ["100"]` | `config/liquidity.toml` | A |
| Un nivel vence a las 500 velas | 500 (`500`) | `config/liquidity.toml` `levels.max_age_bars` | A |
| Barrido: la primera vela que pasa el nivel cierra adentro | `barrido con reversión` | `docs/spec/fase-1-especificacion.md` §5.4.1 | A |
| Roto: la primera vela que pasa el nivel cierra afuera | `roto:` | `docs/spec/fase-1-especificacion.md` §5.4.1 | A |
| El mapa no suma al puntaje | `Solo informativo: no suma al puntaje` | `config/liquidity.toml` | A |
