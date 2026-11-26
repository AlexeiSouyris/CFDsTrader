# 1.2 Largo y corto

> **Nivel 1 · Instrumentos.** Educativo: no es consejo de inversión ni promete ganancias. Todo pasa
> en simulación ([paper trading](../GLOSARIO.md#paper-trading)).

## Objetivo

Calcular el resultado de un [largo y de un corto](../GLOSARIO.md#largo-corto), y saber qué se
puede perder en cada caso.

## La idea

- **Largo:** comprás primero y vendés después. Ganás si el precio **sube**.
- **Corto:** vendés primero y comprás después. Ganás si el precio **baja**. Con un CFD no hace
  falta tener el activo: el contrato es una diferencia de precio.

En nuestros CFD de índices el **tamaño** está en euros por punto. Con tamaño 2, cada punto vale 2 €.

```text
largo:  resultado = (salida − entrada) × tamaño
corto:  resultado = (entrada − salida) × tamaño
```

Es antes de costos. Los costos (spread, slippage, financiación) los restás en el nivel 2.

**Qué se puede perder.**

- En un largo, lo peor teórico es que el precio llegue a cero.
- En un corto, el precio puede subir sin techo: la pérdida teórica **no tiene límite**.
- Por eso el stop es obligatorio en todo el sistema (nivel 7), y por eso existe la **protección de
  saldo negativo**: en la UE (y en España, por la CNMV), un cliente minorista de CFD no puede perder
  más que el dinero de su cuenta de CFD. Eso no evita perder **todo** lo que hay en la cuenta.

Los tamaños mínimos de nuestros CFD: US500 1, US100 0,5, US30 0,2 y DE40 0,5 € por punto.

> **Probá antes de seguir:** abrís un corto y el precio sube 30 puntos. ¿Ganás o perdés?
>
> <details><summary>Ver la respuesta</summary>
>
> Perdés: (entrada − salida) es negativo. Con tamaño 1, perdés 30 € antes de costos.
>
> </details>

## Interactivo

Un deslizador de precio sobre una posición simulada (largo o corto): movés el precio y el motor
calcula el resultado. La UI no hace la cuenta: la pide al motor.

## Ejemplo resuelto

Precios de ejemplo, redondos (no son cotizaciones):

| Caso | Entrada | Salida | Tamaño | Cuenta | Resultado |
|---|---|---|---|---|---|
| Largo US500 | 6.000 | 6.025 | 2 | (6.025 − 6.000) × 2 | **+50 €** |
| Corto US100 | 25.000 | 25.040 | 1 | (25.000 − 25.040) × 1 | **−40 €** |
| Corto DE40 | 24.000 | 23.950 | 3 | (24.000 − 23.950) × 3 | **+150 €** |

## Ejercicio

Cuatro casos, antes de costos (precios de ejemplo):

1. Largo US30, tamaño 0,2, de 44.000 a 44.100.
2. Corto US500, tamaño 1, de 6.000 a 5.980.
3. Largo DE40, tamaño 0,5, de 24.000 a 23.900.
4. Corto US100, tamaño 2, de 25.000 a 25.000.

<details><summary>Ver la solución</summary>

1. (44.100 − 44.000) × 0,2 = **+20 €**.
2. (6.000 − 5.980) × 1 = **+20 €**.
3. (23.900 − 24.000) × 0,5 = **−50 €**.
4. 0 € antes de costos; con costos, **pierde** el spread y el slippage (nivel 2).

</details>

## Examen

5 preguntas. Para aprobar hace falta el 80 % (umbral provisorio).

## Desbloquea

Nada nuevo: prepara el ticket de paper (nivel 7).

## Cifras y fuentes

| Cifra | Valor | Fuente | Prueba |
|---|---|---|---|
| CFD de índices: € por punto (US500) | 1 (`1`) | `config/instruments.toml` `instruments.US500.value_per_point` (API de IG, demo) | A |
| Tamaño mínimo US500 | 1 (`1`) | `config/instruments.toml` `instruments.US500.min_deal_size` | A |
| Tamaño mínimo US100 | 0,5 (`0.5`) | `config/instruments.toml` `instruments.US100.min_deal_size` | A |
| Tamaño mínimo US30 | 0,2 (`0.2`) | `config/instruments.toml` `instruments.US30.min_deal_size` | A |
| Tamaño mínimo DE40 | 0,5 (`0.5`) | `config/instruments.toml` `instruments.DE40.min_deal_size` | A |
| Protección de saldo negativo (UE) | — | [ESMA, aviso ESMA35-43-1135 (2018), artículos 1(f) y 2(c)](https://www.esma.europa.eu/sites/default/files/library/esma35-43-1135_notice_of_pi_decisions_on_cfds_and_binary_options.pdf), leído el 26-11-2026 | A |
| Protección de saldo negativo (España) | — | [Resolución de la CNMV de 27-06-2019 (BOE-A-2019-9737), artículos 3.f y 4.c](https://www.boe.es/buscar/act.php?id=BOE-A-2019-9737), leída el 26-11-2026 | A |
| Largo, corto y cómo se arma una venta en corto | — | [Harris, *Trading and Exchanges*, Oxford University Press (2002/2003)](https://ideas.repec.org/b/oxp/obooks/9780195144703.html) | B |
