# Qué hicimos en CFDsTrader, explicado fácil

Estado al 24-11-2026 (#498). El detalle técnico vive en `docs/estado-de-fases.md` y en `docs/analysis/`. Nada de esto es una recomendación de inversión, y los resultados pasados no garantizan resultados futuros.

## 1. Qué es el proyecto

Es un programa que mira los mercados, busca momentos para comprar o vender y propone operaciones. Funciona **solo en simulación** (*paper*), con dinero de mentira, para ver si una idea sirve antes de arriesgar dinero real.

- **Índice:** un número que resume a muchas empresas juntas. Usamos US500 (las 500 grandes de EE. UU.), US100 (tecnológicas), US30, DE40 (Alemania) y US2000 (empresas chicas de EE. UU.). También el oro.
- **CFD:** un contrato con el que se gana o se pierde según suba o baje el precio, sin comprar la cosa. Tiene costos: el *spread* (la diferencia entre el precio de compra y el de venta) y un interés por cada noche abierta.
- **Backtest:** probar una idea con datos del pasado como si se hubiera usado en ese momento.
- **R:** lo que se arriesga en una operación. "+0,1 R" quiere decir ganar el 10 % de lo arriesgado.

## 2. Cómo está armado

- **Motor (Rust):** precios, patrones, decisiones, riesgo y órdenes simuladas.
- **Inteligencia (Python):** noticias, momentos parecidos del pasado y aprendizaje automático (ML).
- **Base de datos:** todo el historial.
- **Pantalla web:** gráficos, cuenta simulada y botón de emergencia.

Las **reglas anti-autoengaño** son tres: nunca usar información del futuro, contar siempre los costos y guardar unos datos finales sin mirar hasta el final.

## 3. Lo que construimos

1. **Especificación:** qué tiene que hacer el sistema.
2. **Datos:** precios de IG, **15 años de futuros** de Databento, tasas de interés de EE. UU. y Europa, euro/dólar, VIX (el "índice del miedo") y bonos del Tesoro.
3. **Laboratorio de pruebas** (backtest).
4. **Análisis técnico y estrategia:** indicadores, patrones de velas y zonas de liquidez, en gráficos de 15 minutos.
5. **Inteligencia:** noticias, momentos parecidos del pasado y ML.
6. **Riesgo:** como máximo el 1 % de la cuenta por operación, stop-loss obligatorio, sin operaciones nuevas el día que se pierde el 3 % y **botón de emergencia** (*kill switch*) si la cuenta cae un 15 % o si fallan los datos.
7. **Broker simulado** con costos realistas.
8. **Piloto automático de paper:** listo y apagado hasta que vuelvan los datos en vivo de IG.
9. **Monitoreo:** registros y alertas.
10. **Pantalla:** gráficos navegables, medición sobre el gráfico y animación de entrada.

La cuenta simulada arranca con **10.000 €** (con 1.000 € el DE40 rechazaba todas las operaciones).

## 4. Resultados de las pruebas

| Prueba | Resultado | Qué significa |
|---|---|---|
| Estrategia actual, 15 años (US500, US100, US30) | No gana ni **antes** de costos; los costos se llevan 0,10–0,18 R por operación | La idea, tal como está, no tiene ventaja |
| La misma en US2000 y DE40 | También pierde | No es un problema de un solo índice |
| ~300 filtros distintos | Ninguno la da vuelta | La base no tiene ventaja |
| ML v1 | Acierta 0,525 (0,5 es tirar una moneda) | No predice mejor que el azar |
| ML v2 | No mejora | Cerrado por ahora |
| Ronda 1: ruptura del rango de apertura | Pierde 0,5 % por mes | Descartada |
| Ronda 1: efecto "cambio de mes" | +0,03 % por mes, demasiado chico para distinguirlo del azar | Descartada |
| Ronda 2: tendencia mensual | +0,08 % por mes; puede ser azar y rinde menos que comprar y mantener | Descartada |
| Ronda 2: momentum de fin del día | Pierde 2 % por mes, todos los años | Descartada |
| Patrones de "manipulación" | Los delitos no se ven con nuestros datos; los comportamientos normales ya están en el sistema y no muestran ventaja | Dos ideas pasan a la investigación nueva |
| Ensayo del piloto automático | Se encontraron 7 errores y se arreglaron todos | Más seguro antes de prenderlo |

**En resumen:** el laboratorio es sólido y honesto, pero **todavía no apareció una idea que gane después de costos**. Es lo normal: la mayoría de las ideas de trading fallan cuando se prueban en serio, y descubrirlo en simulación no cuesta nada.

## 5. Qué sigue

- **Programa de investigación nuevo** (#493): noticias y eventos, plazos largos, bajar costos, régimen del VIX y los números redondos.
- **Herramientas nuevas** (#491 y #497): Fibonacci y golden pocket, EMA 365, spring de Wyckoff, fondo y números redondos. Sirven para mirar y analizar; no entran en la estrategia sin una prueba que lo justifique.
- **Después del 27-11-2026:** vuelven los datos en vivo de IG, se prende el piloto automático en simulación y corren **3 meses** de paper.
- **Dinero real:** bloqueado. Necesita la aprobación de dos personas y, antes, una idea que funcione.
