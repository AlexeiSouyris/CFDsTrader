# 7.5 Caso real: los topes no crean ventaja

> **Nivel 7 · Riesgo y tamaño.** Educativo: no es consejo de inversión ni promete ganancias. Todo
> pasa en simulación ([paper trading](../GLOSARIO.md#paper-trading)). Son resultados del pasado, con
> datos de futuros y costos del CFD: no dicen nada seguro del futuro.

## Objetivo

Leer un resultado propio que muestra el límite del manejo de riesgo: con los topes, la estrategia en
uso **pierde menos y más despacio, pero sigue perdiendo**. El riesgo no arregla una estrategia sin
ventaja.

## La idea

En R039 corrimos la estrategia en uso (la de R001 a R003, sin ningún cambio) en una sola cuenta con
US500, US100 y US30, de 02-2013 a 10-2025 (153 meses), con 10.000 € de capital inicial. Dos
versiones: **sin** los topes nuevos y **con** los topes que el dueño aprobó (grupo de EE. UU. al 1,5
%, colchón por gaps de 0,25 ATR y reducción gradual por caída).

| | Sin topes | Con topes |
|---|---|---|
| Capital final | 242 € | 841 € |
| Retorno total | −97,6 % | −91,6 % |
| Caída máxima | 97,6 % | 91,6 % |
| Monte Carlo: caída p95 | 99,6 % | 97,4 % |
| Veces que saltó el kill switch | 28 | 16 |
| Sharpe (diario, anualizado) | −1,69 | −1,95 |

(En el backtest, el kill switch se reactiva a los 20 días hábiles para poder seguir midiendo; por
eso salta varias veces. En paper no se reactiva solo.)

**Cómo se lee.**

1. **Pierde con o sin topes.** Los topes no cambian ninguna señal: solo el tamaño. Si cada
   operación pierde en promedio después de costos (R001 a R003, nivel 2), achicarlas no la vuelve
   ganadora.
2. **Con topes pierde menos plata.** Casi todo el efecto viene de la **reducción gradual**: con la
   cuenta en caída, cada operación arriesga menos (en promedio 0,52 % frente a 0,98 %). El tope del
   grupo y el colchón por gaps, solos, casi no mueven el resultado final.
3. **El Sharpe empeora** (−1,95 frente a −1,69). Achicar en las rachas malas reduce la pérdida en
   euros pero no mejora la pérdida por unidad de riesgo. No hay que leerlo como "mejor" o "peor"
   gestión: describe una estrategia que pierde.
4. **Hay más operaciones con topes** (1.352 frente a 978), porque la cuenta se achica más despacio:
   sin topes, el capital cae tanto que muchas señales no llegan al tamaño mínimo del broker.

**Lo que hizo cada tope:** el del grupo achicó 198 operaciones y rechazó 45; el colchón por gaps
achicó todas (en promedio, el colchón es el 14,8 % del riesgo de cada una); la reducción gradual
actuó en 1.073 de las 1.352 operaciones.

**La lección:** el manejo de riesgo sirve para **sobrevivir** a una mala racha y para no apostar
todo a una sola idea. No crea ventaja. Primero hace falta una estrategia que gane después de
costos; después, el riesgo la protege. Es un resultado **descriptivo**: no vota ni suma al número
de pruebas.

> **Probá antes de seguir:** si los topes no hacen ganar, ¿para qué los queremos?
>
> <details><summary>Ver la respuesta</summary>
>
> Para que una estrategia que **sí** tenga ventaja no se funda en una mala racha antes de que la
> ventaja aparezca, y para que varias posiciones parecidas no sean una sola apuesta grande. Con
> una estrategia sin ventaja, solo hacen que el final llegue más despacio.
>
> </details>

## Interactivo

La tabla de R039 con sus cinco variantes (sin topes, solo gaps, solo grupo, solo caída y con
topes) y el enlace a su informe.

## Ejercicio

En la tabla del informe, `solo_caida` termina con 671 € y `solo_grupo` con 279 €. Explicá en una
frase por qué la reducción gradual mueve más el resultado que el tope del grupo.

<details><summary>Ver la solución</summary>

Porque la cuenta pasó casi todo el período en caída, y la reducción gradual achica **todas** esas
operaciones a la mitad o menos; el tope del grupo solo actúa cuando hay varias posiciones de EE. UU.
abiertas a la vez.

</details>

## Examen

Esta lección no tiene examen: es el caso real del nivel. Se completa al leerla.

## Desbloquea

Nada nuevo: el ticket de paper y la barra de estado se abren en 7.3 y 7.4.

## Cifras y fuentes

| Cifra | Valor | Fuente | Prueba |
|---|---|---|---|
| Retorno total con / sin topes | `-91.6 % / -97.6 %` | `docs/analysis/registro-de-resultados.csv` `R039` | A |
| Sharpe con / sin topes | `Sharpe -1.95 / -1.69` | `docs/analysis/registro-de-resultados.csv` `R039` | A |
| Monte Carlo: caída p95 con / sin topes | `MC p95 97.4 % / 99.6 %` | `docs/analysis/registro-de-resultados.csv` `R039` | A |
| Kill switch con / sin topes | `kill switch 16 veces frente a 28` | `docs/analysis/registro-de-resultados.csv` `R039` | A |
| Período | `2013-02 a 2025-10 (153 meses)` | `docs/analysis/registro-de-resultados.csv` `R039` | A |
| Tope del grupo: achicadas y rechazadas | `198 operaciones achicadas y 45 rechazadas` | `docs/analysis/registro-de-resultados.csv` `R039` | A |
| Colchón por gaps: parte del riesgo | `14.8 %` | `docs/analysis/registro-de-resultados.csv` `R039` | A |
| Reducción gradual: operaciones con riesgo reducido | `1073 de 1352` | `docs/analysis/registro-de-resultados.csv` `R039` | A |
| Descriptivo: no vota ni suma al N | `no vota ni suma al N` | `docs/analysis/registro-de-resultados.csv` `R039` | A |
| Capital final sin topes | 242 € (`242`) | `docs/analysis/2026-11-riesgo-con-sin-topes.md` | A |
| Capital final con topes | 841 € (`**841**`) | `docs/analysis/2026-11-riesgo-con-sin-topes.md` | A |
| Riesgo medio por operación con / sin topes | `riesgo medio 0,52 % frente a 0,98 %` | `docs/analysis/2026-11-riesgo-con-sin-topes.md` | A |
| Operaciones con / sin topes | `Hay más operaciones con topes (1 352 frente a 978)` | `docs/analysis/2026-11-riesgo-con-sin-topes.md` | A |
| Capital final de solo caída y solo grupo | `671` | `docs/analysis/2026-11-riesgo-con-sin-topes.md` | A |
| Capital inicial | `10 000 EUR` | `docs/analysis/2026-11-riesgo-con-sin-topes.md` | A |
| Tope del grupo aprobado | 1,5 % (`max_risk_pct = "1.5"`) | `config/risk.toml` | A |
| Colchón por gaps aprobado | 0,25 ATR (`0.25`) | `config/risk.toml` `gap.gap_atr` | A |
