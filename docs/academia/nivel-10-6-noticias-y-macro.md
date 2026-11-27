# 10.6 Noticias y macro

> **Nivel 10 · Tu propia estrategia.** Educativo: no es consejo de inversión ni promete ganancias.
> Todo pasa en simulación ([paper trading](../GLOSARIO.md#paper-trading)). Son resultados del
> pasado: no dicen nada seguro del futuro.

## Objetivo

Usar una noticia o un dato **solo desde su hora de publicación real**, y conocer qué dieron
nuestras pruebas con el FOMC, el CPI y el empleo.

## La idea

**1. La hora que importa es la de publicación.** Una noticia influye desde el momento en que
**estuvo disponible**, no desde la hora del hecho que describe (CLAUDE.md §5b). El comunicado del
FOMC de las 14:00 ET existe a las 14:00, aunque la decisión se haya tomado antes; el CPI existe a
las 08:30 ET. Por eso:

- el calendario oficial del proyecto (FOMC desde la Fed, CPI y empleo desde el BLS) guarda la
  **hora de publicación** de cada dato, y los lectores de noticias guardan la hora real de cada
  noticia;
- los datos fuertes que se tratan aparte en las pruebas son el comunicado del FOMC, el CPI y el
  informe de empleo;
- la estrategia **no abre** una operación si hay una noticia de alto impacto del instrumento o de
  su país entre **15 minutos antes y 15 minutos después**.

**2. La deriva antes del FOMC: la literatura y nuestra prueba.** Lucca y Moench (2015) encontraron
retornos medios grandes de la bolsa de EE. UU. en las horas **antes** de los anuncios programados
del FOMC, una parte importante del retorno anual. Nuestra H1 lo probó en US100 de 2013 a 2025, con
la regla congelada antes:

- antes de costos, el US100 dio **+20 pb** por evento (el US500, +14 pb);
- alfa contra comprar y mantener: **+0,071 %** por mes, con **p 0,120** y Sharpe deflactado
  **0,036**: **no pasa** (R027);
- casi toda la ventaja vino de 2021–2025 (−0,017 % por mes antes y +0,215 % después): justo lo
  que la exigencia de las dos mitades no acepta.

**3. Los días de CPI y empleo (H2, R031).** Entrar a las 08:00 ET del día del dato y salir al final
de la sesión: alfa de **−0,039 %** por mes, p 0,607. **No pasa.** En US500 y US30, el mismo patrón
(R032, descriptivo).

**4. La sorpresa.** Lo que mueve el precio en un dato macro no es el dato, sino **la sorpresa**: el
dato real menos lo que se esperaba (Andersen, Bollerslev, Diebold y Vega, 2003). Sabemos cuándo
sale cada CPI, pero no teníamos cuánto se esperaba. Desde el 25-11-2026 (ADR 0018) el proyecto
guarda el pronóstico y el dato anterior de cada evento del calendario, con la hora en que se vio,
y el motor calcula la sorpresa con el último pronóstico guardado **antes** de la hora del dato (sin
mirar el futuro). No hay historia: cada semana sin guardar era una semana perdida.

**5. Leer noticias no es predecir.** El intérprete de noticias del proyecto acertó el **60,8 %**,
con un intervalo de **47 % a 73 %**: con esa muestra, **no se distingue del azar** (el intervalo
incluye el 50 %). La pestaña Noticias de la Terminal sirve para saber qué pasó y cuándo, no para
saber qué va a pasar.

> **Probá antes de seguir:** un backtest usa el titular «la Fed sube las tasas» con la hora de la
> reunión (13:00 ET) y no la del comunicado (14:00 ET). ¿Qué gana de más?
>
> <details><summary>Ver la respuesta</summary>
>
> Una hora de futuro: opera con información que en vivo recién iba a existir a las 14:00. Si el
> mercado se movió entre las 13:00 y las 14:00 en la dirección del anuncio, el backtest «lo vio
> venir» por un error de horas, no por una ventaja.
>
> </details>

## Interactivo

El calendario con la hora real de cada dato y un replay de un día de CPI: las velas antes de las
08:30 ET y la reacción después, sin ver el dato antes de tiempo.

## Ejercicio

Para cada caso, decí desde qué hora puede usarse la información en un backtest:

1. El informe de empleo de EE. UU. de un viernes.
2. Un discurso de un miembro de la Fed transcripto por un medio una hora después.
3. El pronóstico del CPI que figura en el calendario la semana anterior.

<details><summary>Ver la solución</summary>

1. Desde las 08:30 ET, su hora de publicación.
2. Desde que se publicó lo que vas a usar: si usás la transcripción, desde la hora de la
   transcripción, no la del discurso.
3. Desde la hora en que se guardó ese pronóstico (y solo si lo guardamos en ese momento; un
   pronóstico «de la semana anterior» reconstruido hoy puede tener datos que entonces no estaban).

</details>

## Examen

5 preguntas. Para aprobar hace falta el 80 % (umbral provisorio).

## Desbloquea

- **Noticias y calendario** de la Terminal: las últimas noticias con su hora de publicación real y
  los próximos eventos del calendario.

## Cifras y fuentes

| Cifra | Valor | Fuente | Prueba |
|---|---|---|---|
| Regla: noticias con hora de publicación real | `una noticia solo influye desde el momento en que estuvo disponible` | `CLAUDE.md` | A |
| Hora de los datos de CPI y empleo | 08:30 ET (`08:30`) | `config/research.toml` `macro_day.release_time` | A |
| Entrada de H2 | 08:00 ET (`08:00`) | `config/research.toml` `macro_day.entry_time` | A |
| Datos fuertes | `strong_releases = ["fed_fomc/fomc_statement", "bls/cpi", "bls/employment_situation"]` | `config/research.toml` | A |
| Filtro de noticias de alto impacto | `minutos antes y 15 minutos después` | `docs/spec/fase-1-especificacion.md` | A |
| Calendario oficial con hora de publicación | `published_at` | `docs/modules/calendar.official.md` | A |
| H1: antes de costos | `el US100 +20 pb` | `docs/analysis/2026-11-programa-nasdaq-h1-resultados.md` | A |
| H1: alfa, p, DSR y mitades (USD) | `+0.071 % (p 0.120; DSR 0.036; -0.017 % / +0.215 %)` | `docs/analysis/registro-de-resultados.csv` `R027` | A |
| H2: alfa y p (USD) | `-0.039 % (p 0.607` | `docs/analysis/registro-de-resultados.csv` `R031` | A |
| H2 en US500 y US30 | `mismo patrón que US100 y ninguno cerca de pasar` | `docs/analysis/registro-de-resultados.csv` `R032` | A |
| Se guarda el pronóstico desde el 25-11-2026 | `Guardar el pronóstico, el dato anterior y el dato real del calendario` | `docs/decisions/0018-sorpresa-del-calendario.md` | A |
| El pronóstico se guarda con la hora en que se vio | `fila nueva solo cuando el par cambia, con` | `docs/modules/calendar.md` | A |
| La sorpresa usa el pronóstico guardado antes del dato | `estrictamente anterior a la hora del evento` | `docs/modules/market-data.md` | A |
| La sorpresa, no el dato | `Andersen, Bollerslev, Diebold y Vega 2003` | `docs/decisions/0018-sorpresa-del-calendario.md` | A |
| Acierto del intérprete de noticias | `60,8 %, con un intervalo de [47, 73] %, no distinguible del azar` | `docs/analysis/2026-11-autoanalisis.md` | A |
| Deriva antes del FOMC | — | [Lucca y Moench (2015), *The Pre-FOMC Announcement Drift*, Journal of Finance 70(1), 329–371](https://ideas.repec.org/a/bla/jfinan/v70y2015i1p329-371.html), resumen leído el 26-11-2026 | A |
