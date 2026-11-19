# Fase 1 — Preguntas para cerrar la especificación

Respondan **los dos** debajo de cada pregunta (o en el chat con su agente, que lo pasa acá).
Cuando no queden preguntas abiertas, el agente redacta `docs/spec/fase-1-especificacion.md` con reglas en pseudocódigo para aprobar.
Objetivo: **dos personas que lean la spec harían exactamente las mismas operaciones.**

> ¿No conocés un término? Hacé clic sobre él: todos llevan al [Glosario](../GLOSARIO.md).

Estado: ✅ respondida · ⏳ pendiente

## A. Mercado y broker
1. ✅ **Instrumentos:** índices de EE. UU. (US Tech 100, US 500, Wall Street), acciones puntuales e índices de Europa/Asia. Todos como [CFD](../GLOSARIO.md#cfd).
2. ✅ **Broker:** Revolut u otro. ⚠️ Revolut no tiene API pública de trading, así que el sistema no puede leer ni ejecutar ahí. Ver la sección K.
3. ⏳ ¿País de residencia de los miembros? Define qué brokers están disponibles y el [apalancamiento](../GLOSARIO.md#apalancamiento) máximo permitido ([regulador](../GLOSARIO.md#esma)).
4. ⏳ ¿De dónde salen los datos históricos y en qué resolución? ¿Hay presupuesto para datos pagos?
5. ✅ **[Timeframes](../GLOSARIO.md#timeframe):** entrada en **15m**, [tendencia](../GLOSARIO.md#tendencia) en **1h**.
6. ⏳ ¿En qué horario opera el sistema y en qué zona horaria?
7. ✅ **Overnight:** se puede dejar abierto de noche (pagando [swap](../GLOSARIO.md#swap)), pero **todo se cierra antes del cierre del viernes** para evitar el [gap](../GLOSARIO.md#gap) del fin de semana.

## B. Rol del sistema
8. ⏳ ¿El sistema sugiere y ustedes ejecutan a mano, o tiene que ejecutar solo en un broker con API? (depende de la sección K)
9. ⏳ Cuéntenme paso a paso **una operación real** que hayan hecho.

## C. Entrada
10. ✅ **Señales de la estrategia base**, combinadas:
    - Velas: [martillo](../GLOSARIO.md#martillo) y [envolvente](../GLOSARIO.md#envolvente) con filtro [SMA50](../GLOSARIO.md#sma).
    - [Doble techo / doble suelo](../GLOSARIO.md#doble-techo).
    - [Liquidez](../GLOSARIO.md#liquidez) según el marco ICT/SMC.
    - [Soportes](../GLOSARIO.md#soporte) y [resistencias](../GLOSARIO.md#resistencia).
    - ⏳ Falta definir cómo se combinan: ¿alcanza con una señal o hacen falta varias en confluencia?
11. ✅ **Doble techo o suelo:** se entra al **cerrar la vela de 15m que rompe el [neckline](../GLOSARIO.md#neckline)**. ⏳ ¿Qué diferencia máxima entre los dos topes (en %)?
12. ✅ **Liquidez:** marco ICT/SMC estándar: [externa](../GLOSARIO.md#liquidez-externa), [interna](../GLOSARIO.md#liquidez-interna), [sweeps](../GLOSARIO.md#sweep), [order blocks](../GLOSARIO.md#order-block), [FVG](../GLOSARIO.md#fvg), [BOS](../GLOSARIO.md#bos) y [CHoCH](../GLOSARIO.md#choch).
13. ⏳ ¿Las opciones (volatilidad implícita, put/call) son señal o solo contexto?
14. ⏳ ¿Qué tiene que pasar para **no** entrar?

## D. Salida
15. ⏳ [Stop loss](../GLOSARIO.md#stop-loss): ¿por estructura, por [ATR](../GLOSARIO.md#atr) o un % fijo?
16. ⏳ [Take profit](../GLOSARIO.md#take-profit): ¿múltiplo de [R](../GLOSARIO.md#r-multiplo), siguiente zona de liquidez, parcial o [trailing](../GLOSARIO.md#trailing-stop)?
17. ⏳ ¿Hay salida por tiempo o antes de noticias?

## E. Tamaño, apalancamiento y riesgo
18. ⏳ Capital inicial y moneda.
19. ⏳ [Riesgo por operación](../GLOSARIO.md#riesgo-por-operacion), pérdida máxima diaria, [drawdown](../GLOSARIO.md#drawdown) máximo y posiciones abiertas máximas.
20. ⏳ ¿Cómo se elige el apalancamiento y cuál es el máximo por instrumento?
21. ⏳ ¿A qué nivel hace [stop-out](../GLOSARIO.md#stop-out) el broker?

## F. Noticias
22. ⏳ ¿Las noticias funcionan como filtro, como ajuste de [probabilidad](../GLOSARIO.md#probabilidad) o como señal propia?
23. ⏳ ¿Qué fuentes y con qué presupuesto?
24. ⏳ ¿La interpretación la hace un LLM, un modelo propio o los dos? ¿En qué idioma son los resúmenes?

## G. Análogos históricos
25. ⏳ ¿Qué es una "situación similar"? ([análogos](../GLOSARIO.md#analogos))
26. ⏳ ¿Qué universo y cuántos años de historia?

## H. "Matemática cuántica"
27. ⏳ ¿Computación cuántica real o modelos probabilísticos avanzados?

## I. Costos y casos borde
28. ⏳ [Spread](../GLOSARIO.md#spread), [comisión](../GLOSARIO.md#comision) y [swap](../GLOSARIO.md#swap) de cada instrumento.
29. ⏳ ¿Qué hacer ante [gaps](../GLOSARIO.md#gap), feriados, [halts](../GLOSARIO.md#halt), datos faltantes, ejecuciones parciales o desconexión?

## J. Proceso
30. ⏳ ¿Cuánto tiempo mínimo de [paper trading](../GLOSARIO.md#paper-trading)? ¿Quién aprueba cada fase?

## K. Fondo del grupo (nuevo)
Son **6 personas**. Cada una aporta un monto distinto cada semana, todo se invierte junto y se calcula el [P&L](../GLOSARIO.md#pl) de cada uno según lo que puso y cuándo.

- ✅ **Cómo se calcula el P&L de cada uno:** con [contabilidad por unidades](../GLOSARIO.md#unidades), como un fondo común. Cada aporte compra unidades al [valor del día](../GLOSARIO.md#nav). **Esto lo hace nuestro sistema** (módulo `fund` en Rust), sin importar el broker.
31. ⏳ ¿A nombre de quién está la cuenta del broker? (una persona que actúa de titular, una sociedad o club de inversión, u otra figura)
32. ⏳ ¿Cómo y cuándo se puede retirar dinero? (aviso previo, retiro mínimo, al valor de la unidad de ese día)
33. ⏳ ¿Quién decide las operaciones? (el sistema y un responsable, votación del grupo, cualquiera de los 6)
34. ⏳ ¿Aporte mínimo, día fijo de aporte y moneda?

> ⚠️ Manejar dinero de varias personas en una cuenta puede tener consecuencias legales e impositivas según el país (en algunos casos se considera gestión de fondos de terceros). Conviene consultarlo con un contador o abogado antes de juntar plata real. El sistema funciona igual en simulación mientras tanto.
