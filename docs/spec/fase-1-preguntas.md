# Fase 1 — Preguntas para cerrar la especificación

Respondan debajo de cada pregunta (o en el chat con su agente, que lo pasa acá).
Cuando no queden preguntas abiertas, el agente redacta `docs/spec/fase-1-especificacion.md` con reglas en pseudocódigo para aprobar.
Objetivo: **dos personas que lean la spec harían exactamente las mismas operaciones.**

> ¿No conocés un término? Hacé clic sobre él: todos llevan al [Glosario](../GLOSARIO.md).

> **Cerrado.** El grupo respondió lo que es decisión del grupo, y el agente resolvió las preguntas técnicas restantes (a pedido del grupo, que no es experto en trading). El resultado está en [`fase-1-especificacion.md`](fase-1-especificacion.md), sección 16, que separa qué decidió cada uno. Este archivo queda como historial; algunas marcas ⏳ de abajo quedaron resueltas directamente en la especificación.

Estado: ✅ respondida · ⏳ pendiente

## A. Mercado y broker
1. ✅ **Instrumentos:** índices de EE. UU. (US Tech 100, US 500, Wall Street), acciones puntuales e índices de Europa/Asia. Todos como [CFD](../GLOSARIO.md#cfd).
2. ✅ **Broker:** hoy usan Revolut, que no tiene API pública de trading. Como los miembros están en la UE, hay opciones con API y cuenta demo, por ejemplo IG (REST + streaming). La elección final va en un ADR.
3. ✅ **País:** España / UE → regulación [ESMA](../GLOSARIO.md#esma): índices principales hasta 20:1, acciones hasta 5:1.
4. ⏳ ¿De dónde salen los datos históricos y con qué presupuesto?
5. ✅ **[Timeframes](../GLOSARIO.md#timeframe):** entrada en **15m**, [tendencia](../GLOSARIO.md#tendencia) en **1h**.
6. ⏳ ¿En qué horario opera el sistema? (zona horaria de referencia: Europe/Madrid para mostrar, UTC internamente)
7. ✅ **Overnight:** se puede dejar abierto de noche (pagando [swap](../GLOSARIO.md#swap)), pero **todo se cierra antes del cierre del viernes** para evitar el [gap](../GLOSARIO.md#gap) del fin de semana.

## B. Rol del sistema
8. ✅ **El sistema propone y un responsable designado aprueba y ejecuta.** Mientras tanto, el sistema lleva un P&L simulado ([paper trading](../GLOSARIO.md#paper-trading)).
9. ⏳ (Opcional, muy útil) Cuéntenme paso a paso **una operación real** que hayan hecho.

## C. Entrada
10. ✅ **Señales de la estrategia base:** velas ([martillo](../GLOSARIO.md#martillo), [envolvente](../GLOSARIO.md#envolvente)) con filtro [SMA50](../GLOSARIO.md#sma), [doble techo o suelo](../GLOSARIO.md#doble-techo), [liquidez](../GLOSARIO.md#liquidez) ICT/SMC, y [soportes](../GLOSARIO.md#soporte) y [resistencias](../GLOSARIO.md#resistencia).
    - ✅ **Cómo se combinan:** por **puntaje de confluencia**. Cada señal suma puntos (incluida la tendencia de 1h a favor) y se entra si se supera un umbral configurable. Así se puede medir cuánto aporta cada señal.
11. ✅ **Doble techo o suelo:** se entra al **cerrar la vela de 15m que rompe el [neckline](../GLOSARIO.md#neckline)**. La tolerancia entre los dos topes es un parámetro (inicial: 0,3%) que se calibra con el test de sensibilidad.
12. ✅ **Liquidez:** marco ICT/SMC estándar: [externa](../GLOSARIO.md#liquidez-externa), [interna](../GLOSARIO.md#liquidez-interna), [sweeps](../GLOSARIO.md#sweep), [order blocks](../GLOSARIO.md#order-block), [FVG](../GLOSARIO.md#fvg), [BOS](../GLOSARIO.md#bos) y [CHoCH](../GLOSARIO.md#choch).
13. ⏳ ¿Las opciones (volatilidad implícita, put/call) son señal o solo contexto?
14. ✅ (en parte) **No entrar** cerca de noticias de alto impacto (ver F). ⏳ ¿Otros filtros?

## D. Salida
15. ✅ **[Stop loss](../GLOSARIO.md#stop-loss):** detrás de la estructura (mínimo o máximo del patrón, o la mecha del [sweep](../GLOSARIO.md#sweep)) **+ 0,5 × [ATR](../GLOSARIO.md#atr)**.
16. ✅ **[Take profit](../GLOSARIO.md#take-profit):** se cierra el **50% en 2[R](../GLOSARIO.md#r-multiplo)**, el stop pasa al precio de entrada y el resto sigue con [trailing stop](../GLOSARIO.md#trailing-stop).
17. ✅ (en parte) Cierre obligatorio antes del cierre del viernes. Alerta de salida ante una noticia fuerte en contra (ver F). ⏳ ¿Salida por tiempo?

## E. Tamaño, apalancamiento y riesgo
18. ⏳ Capital inicial simulado y moneda.
19. ✅ **Perfil moderado:**
    - [Riesgo por operación](../GLOSARIO.md#riesgo-por-operacion): **1%**.
    - Pérdida máxima diaria: **-3%**.
    - [Kill switch](../GLOSARIO.md#kill-switch) con [drawdown](../GLOSARIO.md#drawdown) del **-15%**.
    - Hasta **5 posiciones** abiertas.
20. ✅ **[Apalancamiento](../GLOSARIO.md#apalancamiento):** sale del riesgo. El tamaño se calcula con el 1% y la distancia al stop, y el apalancamiento resultante tiene un **tope configurable por instrumento** (inicial: 10:1 en índices y 3:1 en acciones, por debajo de ESMA).
21. ⏳ Nivel de [stop-out](../GLOSARIO.md#stop-out) del broker. Se toma de la documentación oficial del broker elegido; con ESMA es el 50% del margen.

## F. Noticias
22. ✅ **Las noticias cumplen cuatro roles:**
    1. **Filtro**: no abrir operaciones cerca de noticias de alto impacto.
    2. **Ajuste de [probabilidad](../GLOSARIO.md#probabilidad)**: el [sentimiento](../GLOSARIO.md#sentimiento) suma o resta al puntaje.
    3. **Alerta de salida**: una noticia fuerte en contra de una posición abierta avisa al responsable.
    4. **Señal propia**: va en fase experimental, separada, y solo pasa a usarse si demuestra valor fuera de muestra.
23. ⏳ ¿Qué fuentes y con qué presupuesto?
24. ✅ **Interpretación con un LLM (Claude):** resumen, si la noticia es buena o mala, para qué activos y con qué magnitud. ⏳ ¿En qué idioma son los resúmenes?

## G. Análogos históricos
25. ⏳ ¿Qué es una "situación similar"? ([análogos](../GLOSARIO.md#analogos))
26. ⏳ ¿Qué universo y cuántos años de historia?

## H. "Matemática cuántica"
27. ✅ **Modelos probabilísticos:** Monte Carlo, modelos bayesianos, cadenas de Markov y ML. No se usa computación cuántica real.

## I. Costos y casos borde
28. ⏳ [Spread](../GLOSARIO.md#spread), [comisión](../GLOSARIO.md#comision) y [swap](../GLOSARIO.md#swap): se toman de la documentación del broker elegido.
29. ⏳ ¿Qué hacer ante [gaps](../GLOSARIO.md#gap), feriados, [halts](../GLOSARIO.md#halt), datos faltantes o desconexión?

## J. Proceso
30. ⏳ ¿Cuánto tiempo mínimo de [paper trading](../GLOSARIO.md#paper-trading) antes de usar dinero real?

## K. Fondo del grupo
Son **6 personas** que aportan montos distintos cada semana. Todo se invierte junto y se calcula el [P&L](../GLOSARIO.md#pl) de cada uno.
- ✅ **P&L de cada miembro:** [contabilidad por unidades](../GLOSARIO.md#unidades). Cada aporte compra unidades al [valor del día](../GLOSARIO.md#nav). Lo calcula el módulo `fund` de nuestro sistema.
31. ✅ **Titular:** una persona abre la cuenta del broker a su nombre y nuestro sistema lleva la contabilidad de los 6.
32. ✅ **Retiros:** semanales, pedidos antes del día de aportes y pagados al valor de la unidad de ese día.
33. ✅ **Decisiones:** el sistema propone y uno o dos responsables designados aprueban y ejecutan.
34. ⏳ ¿Aporte mínimo, día fijo de aportes y moneda?

> ⚠️ Con un titular que maneja dinero de otras 5 personas, en España puede haber consecuencias legales (gestión de fondos de terceros) e impositivas (el titular declara toda la ganancia). Consulten con un asesor antes de juntar dinero real. Mientras tanto todo funciona en simulación.
