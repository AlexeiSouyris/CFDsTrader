# Guía de uso

Esta guía explica cómo prender el sistema y qué muestra cada pantalla. Está pensada para usarlo, no para programarlo. Las palabras técnicas llevan un enlace al [glosario](../GLOSARIO.md).

> **Todo es SIMULACIÓN.** Ninguna pantalla mueve dinero real. Las órdenes del Terminal van a un **broker simulado** dentro del motor ([paper trading](../GLOSARIO.md#paper-trading-simulación)) y nunca salen a IG ni a ningún broker. Operar con dinero real está bloqueado a propósito: solo se podrá habilitar con una decisión firmada por dos personas (fase 11 del ROADMAP). Donde veas **SIMULACIÓN** o **MODO SIMULACIÓN**, quiere decir eso: precios y reglas de verdad, dinero de mentira.
>
> Los números son resultados **del pasado**. No son promesas ni recomendaciones.

---

## 1. Prender el sistema

Se hace en el Mac mini, con la Terminal de macOS, desde la carpeta del proyecto.

1. **Abrí Docker Desktop** y esperá a que diga que está corriendo.
2. **Levantá la base de datos** (PostgreSQL y Redis):
   ```text
   docker compose -f infra/docker-compose.yml --env-file .env up -d
   ```
   Para ver que estén bien, `docker compose -f infra/docker-compose.yml ps`: los dos tienen que decir `healthy`. Desde entonces la base vuelve a levantarse sola cada vez que arranca Docker Desktop (salvo que la hayas parado a mano con `docker compose … stop`). Si el contenedor se había creado antes de este cambio, correr este comando una vez lo actualiza.
3. **Prendé el motor y la pantalla**:
   ```text
   infra/scripts/ui_liquidez.sh
   ```
   El script prende el *gateway*, que es la parte del motor que habla con la pantalla, y la pantalla misma. La primera vez tarda unos minutos en compilar. Cuando termina, dice `✅ gateway en http://127.0.0.1:8787` y abre el navegador solo.
4. Si el navegador no se abre, entrá a **http://localhost:5173**.

**Para apagar:** en la ventana de la Terminal donde corre el script, apretá `Ctrl+C`. Se apagan el gateway y la pantalla. La base de datos sigue prendida; no hace falta apagarla.

**Después de bajar código nuevo**, antes de prender, actualizá la base una vez:
```text
cd engine && cargo run -q -p cfds-storage --example migrate
```
El gateway **no** actualiza la base: su conexión es de solo lectura, a propósito. Este comando agrega las tablas nuevas que falten, sin tocar los datos, y muestra en qué versión quedó la base. Se puede correr las veces que quieras.

---

## 2. Las pestañas

Arriba hay tres pestañas principales: **Performance**, **Panel** y **Terminal**. El resto de las vistas (Liquidez, Glosario, Fondo y otras) se abren desde el menú o desde el **buscador** (ver la sección 3).

Cuando algo dice **"datos de ejemplo"** o **"Prototipo con datos de ejemplo"**, es una maqueta y no viene del motor. Cuando dice **"del motor"**, son datos reales de la simulación.

### Panel

Un resumen de todo en una sola página.

- **Arriba:** el [valor de la unidad](../GLOSARIO.md#valor-de-la-unidad-nav) del fondo, el [P/L](../GLOSARIO.md#pl-pl-ganancias-y-pérdidas) de la semana, el [drawdown](../GLOSARIO.md#drawdown) máximo y las propuestas que esperan al responsable.
- **Índices:** cómo vienen los cuatro índices operables (US Tech 100, US 500, Wall Street y Germany 40).
- **Operación:** el estado del broker simulado.
  - si acepta órdenes;
  - el [kill switch](../GLOSARIO.md#kill-switch);
  - cuánto de la pérdida máxima del día ya se usó;
  - los **eventos del broker y las alertas**, del más nuevo al más viejo. Se guardan en memoria: al reiniciar el gateway empiezan de cero.
- **Contexto de mercado** (solo informativo: no suma al puntaje de las propuestas):
  - el [índice de sentimiento](../GLOSARIO.md#índice-de-sentimiento);
  - los **estados emocionales** del mercado ([FOMO](../GLOSARIO.md#fomo), [capitulación](../GLOSARIO.md#capitulación-pánico-clímax-de-venta), [rechazo](../GLOSARIO.md#rechazo), [distribución](../GLOSARIO.md#distribución) y [complacencia](../GLOSARIO.md#complacencia)), medidos con reglas sobre lo que ya pasó;
  - los [análogos históricos](../GLOSARIO.md#análogos-históricos): situaciones parecidas del pasado y qué pasó después, como una distribución y no como un pronóstico.
- **Liquidez:** el mismo mapa que la vista de Liquidez, en chico. Las tablas de FVG muestran los 8 más nuevos; el botón "Mostrar los N restantes" despliega el resto.

En el Contexto de mercado, si no elegiste instrumento, se abre en el primero que tiene índice de sentimiento. Cada tarjeta dice si no pudo leer sus datos ("no se pudo leer"), que no es lo mismo que "sin datos guardados". Los números largos se muestran redondeados a 2 decimales; el valor guardado no cambia.

### Liquidez

Muestra dónde probablemente hay stops ([liquidez](../GLOSARIO.md#liquidez)) y qué pasó con cada nivel: intacto, [barrido](../GLOSARIO.md#sweep-barrido-de-liquidez) con reversión, roto o vencido. También muestra los [FVG](../GLOSARIO.md#fair-value-gap-fvg) abiertos o llenos y sus estadísticas. Lo calcula el motor con [velas cerradas](../GLOSARIO.md#vela-cerrada).

- Se elige el instrumento (DE40, US100, US30 o US500) y el [timeframe](../GLOSARIO.md#timeframe) (15m, 1h o 1d).
- Si hay velas de futuros, también se elige la **fuente**: "IG CFD" o "futuros". Los futuros tienen una base distinta a la del CFD.
- Es una capa informativa: todavía no suma al puntaje.

### Terminal

Donde se opera **en simulación**.

- **Barra de estado** (arriba):
  - si el broker acepta órdenes;
  - el kill switch (apagado o ACTIVO);
  - la pérdida del día usada sobre el máximo (3 % del capital);
  - el [apalancamiento](../GLOSARIO.md#apalancamiento) y las posiciones abiertas (máximo 5);
  - la última reconciliación.
- **Aviso del kill switch:** si se activa, aparece un aviso rojo, "Kill switch activo: no se opera". Desde ahí no se abre nada. Reactivarlo pide la votación de la spec (§1.2): no se hace desde la pantalla.
- **Tarjetas de cuenta:** capital, P/L abierto, P/L del día, [margen](../GLOSARIO.md#margen), apalancamiento y pérdida del día usada.
- **Gráficos:** con los botones **1, 2 y 4** se ven uno, dos o cuatro gráficos a la vez. En cada uno se elige el instrumento y el timeframe. Se mueven, se acercan y se miden como se explica en la sección 3.
- **Ticket** (para mandar una orden simulada):
  1. Elegí **Comprar** ([largo](../GLOSARIO.md#largo--corto)) o **Vender** (corto) y el instrumento.
  2. Escribí el **stop**, que es obligatorio ([stop loss](../GLOSARIO.md#stop-loss-sl)). El **TP1** es opcional ([take profit](../GLOSARIO.md#take-profit-tp)).
  3. Apretá **"Validar con el motor"**. El motor revisa el riesgo y **calcula el tamaño**: el tamaño no se elige a mano. Si algo no cumple (por ejemplo, el stop está muy lejos), lo rechaza y dice por qué.
  4. Si lo aceptó, apretá **"Comprar en simulación"** (o "Vender en simulación"). En el celular pide una confirmación más: "¿Enviar esta orden al broker simulado?".

  **"Sin velas en vivo desde …"**: si el motor no tiene precios recientes del instrumento, el ticket no muestra un rechazo rojo sino un aviso gris con un reloj: "Sin velas en vivo desde *fecha y hora UTC*. Vuelven cuando se renueve el cupo de IG." El encabezado dice "En espera de datos" y no se puede enviar la orden. No es un error tuyo ni del ticket: pasa cuando se gastó el cupo diario de pedidos a IG. Hay que esperar a que se renueve y volver a validar.
- **Posiciones abiertas:** con su P/L, calculado por el motor. En el celular cada posición se ve como una tarjeta, con el nombre de cada dato. Mientras carga dice que está pidiendo los datos (no "no hay posiciones").
- **Propuestas del motor:** cada una muestra entrada, stop, TP1, tamaño y [puntaje](../GLOSARIO.md#probabilidad-de-la-señal), y la probabilidad de tocar TP1 antes que el stop.
  - Esa probabilidad es una **frecuencia del pasado**, con su rango. Con menos de 30 casos dice "Datos insuficientes".
  - Las propuestas vencen a los 10 minutos.

### Performance

Cómo le fue a la simulación, con costos incluidos: [spread](../GLOSARIO.md#spread), [slippage](../GLOSARIO.md#slippage-deslizamiento) y [financiación](../GLOSARIO.md#swap--financiación-overnight).

- **Corridas de backtest:** cada [backtest](../GLOSARIO.md#backtest) guardado, con sus columnas:
  - operaciones, retorno y drawdown máximo;
  - [Sharpe](../GLOSARIO.md#sharpe--sortino) y acierto ([win rate](../GLOSARIO.md#win-rate));
  - si se activó el kill switch.
  - En el celular y la tableta cada corrida se ve como una tarjeta.
- **Detalle de una corrida:** se abre con "Ver el detalle" y la página baja sola hasta él. Muestra:
  - la curva de capital y la de drawdown;
  - las métricas;
  - comprar y mantener del mismo período, con los mismos costos;
  - los eventos del kill switch y cada operación. Las operaciones se ven de a 25: "Mostrar 25 más" o "Mostrar todas". Los precios de salida se muestran con los mismos decimales que la entrada; si dejás el mouse encima, aparece el valor exacto.
- **Walk-forward / fuera de muestra:** dentro del detalle. Muestra el último reporte de la evaluación de ese instrumento:
  - cada ventana de [walk-forward](../GLOSARIO.md#walk-forward), con entrenamiento y prueba lado a lado;
  - la tabla de **sensibilidad** de los parámetros. Si un parámetro "cambia de signo", aparece resaltado: el resultado depende mucho de ese valor;
  - el tramo [fuera de muestra](../GLOSARIO.md#datos-fuera-de-muestra), aparte y sin retocar.
  - Si no hay reporte, lo dice. Hoy DE40 todavía no tiene datos suficientes para generarlo.
- **Paper vs backtest:** compara el broker simulado con el backtest del mismo período y explica la diferencia por causa. Por ejemplo, precio de ejecución, tamaño, swap o una orden que falta.
- **Aprendizaje:** el informe del diario.
  - Trae la **calibración** (¿la probabilidad mostrada se cumplió?), el aporte de cada señal en [R](../GLOSARIO.md#r-múltiplo-de-riesgo) y las propuestas de cambio.
  - Las propuestas de cambio **quedan pendientes de voto**: el sistema no cambia solo.
- **Fondo:** el [valor de la unidad](../GLOSARIO.md#valor-de-la-unidad-nav), cuántas unidades hay y, en "Ver el fondo y cada miembro", lo de cada uno ([contabilidad por unidades](../GLOSARIO.md#contabilidad-por-unidades-cuotapartes)).

---

## 3. Herramientas de la pantalla

### Los gráficos

Los gráficos de velas (en el Terminal, en Liquidez y en el resto de las vistas con precio) se manejan como en una plataforma de trading. Todo lo de esta parte es **solo para mirar**: mover, acercar o medir no manda nada al motor ni cambia ningún cálculo.

**Moverse por el gráfico**

| Qué querés hacer | Con el mouse o el trackpad | En el celular |
|---|---|---|
| Ir al pasado o volver | Apretá sobre el gráfico y arrastrá hacia los costados. Al soltar sigue un poco por inercia. En el trackpad también sirve deslizar dos dedos de lado. | Arrastrá con un dedo. |
| Acercar o alejar | La **rueda** del mouse sobre el gráfico. Acerca hacia donde está el puntero. En el trackpad, **pellizcá**. | Pellizcá con dos dedos. |
| Volver a lo más nuevo | El botón **"Última vela"** (una flecha hacia la derecha), arriba a la derecha del gráfico. Solo aparece cuando la última vela no se ve. | El mismo botón (en el celular es solo la flecha). |

Cuando el mouse está sobre el gráfico, la rueda acerca o aleja en vez de bajar la página. Para bajar la página, usá la rueda fuera del gráfico.

Los botones de rango de arriba (por ejemplo **"1 día"**, **"3 días"**, **"1 semana"**, **"3 meses"** o **"Todo"**, según el timeframe) ponen el gráfico en ese tramo. Si después lo movés a mano, ningún botón queda marcado y arriba dice cuántas velas estás viendo.

**Más historia:** en el Terminal y en Liquidez, cuando llegás al borde izquierdo arrastrando, el gráfico pide velas más viejas al motor. Mientras tanto dice "Trayendo velas anteriores…". Si no hay más, dice "No hay velas más antiguas guardadas". Si falló, ofrece "reintentar". Solo se agregan velas: los niveles de liquidez y los FVG son los del momento actual.
Si el gateway se prendió con código anterior a esta función, siempre dice "No hay velas más antiguas guardadas": apagalo con `Ctrl+C` y volvé a correr el script.

**La escala de precio** (el eje de la derecha)

- Sola, se ajusta a las velas que se ven: es la **escala automática**.
- Si arrastrás hacia arriba o hacia abajo **sobre el eje de precio**, el gráfico se estira o se achica en vertical. Pasa a **escala manual**: en ese modo, arrastrar el gráfico hacia arriba o abajo también mueve el precio.
- Para volver a la automática, apretá el botón **"Escala auto"** que aparece arriba, o hacé **doble clic sobre el eje de precio**.

**Con el teclado:** primero hacé clic en el gráfico (o llegá con la tecla Tab).

| Tecla | Qué hace |
|---|---|
| `←` / `→` | Mueve el gráfico al pasado o al presente. |
| `+` / `−` | Acerca o aleja. |
| `Fin` | Va a la última vela. |
| `Inicio` | Va a la primera vela cargada. |
| `Shift` + `←` / `→` | Lee vela por vela (muestra los datos de cada una). |
| `Esc` | Borra la medición. |

**Velas o línea:** arriba de los gráficos de precio hay un selector **Velas / Línea**.

- **Velas** muestra cada vela con su apertura, máximo, mínimo y cierre.
- **Línea** une solo los cierres y pinta debajo un área que se desvanece hacia abajo: verde si lo que se ve subió y roja si bajó.

**La cabecera del gráfico** muestra el precio grande y, debajo, cuánto cambió en el tramo que se ve (por ejemplo `+312,3 ▲ 4,49 % · 1 semana`). Al pasar el puntero muestra el cierre de esa vela y su fecha.

**El punto que sigue la vela:** al pasar el mouse (o el dedo) por el gráfico, un punto chico se pone sobre el **cierre** de la vela que está debajo, sin importar a qué altura esté el puntero. En el gráfico de liquidez lo acompaña una línea vertical tenue. Al salir del gráfico desaparece.

**Medir**

Arrastrar solo ya no mide: mueve el gráfico. Para medir hay dos formas:

1. **`Shift` + arrastrar** (con el mouse o el trackpad), o
2. apretá el botón de la **regla** (arriba a la derecha del gráfico; queda resaltado) y después arrastrá. En el celular, la regla es la única forma de medir. Apretala otra vez para apagarla.

Aparece un rectángulo, verde si el precio subió y rojo si bajó entre los dos puntos. Cada punta se pega al **cierre** de su vela y lleva un punto. Pegada al rectángulo hay una **pestaña** del mismo color con la diferencia: en precio, en %, en cantidad de velas y en tiempo (por ejemplo `+431,3 · +2,28 % · 30 velas · 42 d`). La pestaña va arriba del rectángulo, o abajo si no entra, y nunca tapa las velas de adentro. En un gráfico angosto (celular) ocupa dos renglones.

En el modo Línea no hay rectángulo: el tramo medido se resalta con el color del signo, lo de afuera se atenúa y la cabecera muestra el cambio y las dos fechas.

Un clic sin arrastrar o la tecla `Esc` borra la medición. Es solo una medida visual: no manda nada al motor.

### El buscador

Arriba hay un botón **"Buscar una vista o un instrumento"**. También se abre con `⌘ K` en la Mac o `Ctrl K` en otras computadoras.

- Escribí parte del nombre de una vista (por ejemplo "liquidez") o de un instrumento (por ejemplo "US100").
- Movete por la lista con `↑` / `↓` y apretá `Enter` para elegir. `Enter` sin moverte elige la primera opción. `Esc` cierra el buscador.
- También podés hacer clic o tocar una opción.
- Si elegís un **instrumento**, se abre el Terminal con ese instrumento en el primer gráfico.
- Un clic fuera del cuadro del buscador lo cierra sin elegir nada.

### Otras herramientas

- **Cambiar de pestaña sin hacer clic:**
  - en el **trackpad**, deslizá con dos dedos hacia los costados;
  - en el **teclado**, `Ctrl+←/→` o `⌘+[` / `⌘+]`;
  - en el **celular**, deslizá el dedo de lado (no desde el borde de la pantalla).

  Sobre un gráfico, esos deslizamientos mueven el gráfico y no cambian de pestaña: hacelo fuera del gráfico.
- **Grilla de gráficos 1 / 2 / 4** (Terminal): en el celular se ve un gráfico por vez, y con "Gráfico a la vista" se elige cuál.
- **Instalarla como app:**
  - en el iPhone, desde Safari: Compartir → "Agregar a pantalla de inicio";
  - en la Mac, desde Chrome: el ícono de instalar en la barra de direcciones.

  La app guarda solo su armazón. Precios, cuenta y órdenes siempre se piden al motor en el momento: si el motor está apagado, no muestra datos viejos como si fueran de ahora.

> Hoy el gateway solo atiende en esta computadora (`127.0.0.1`). Para usarlo desde el celular hay que decidir antes cómo abrir el acceso (está en la lista de decisiones de [estado-de-fases](../estado-de-fases.md)).

---

## 4. Las variables que configurás vos

Van en el archivo `.env` de la carpeta del proyecto. **Nunca se suben al repositorio ni se comparten.** Acá solo están los nombres; los valores los ponés vos.

| Variable | Para qué | Si no está |
|---|---|---|
| `CFDS_GATEWAY_TOKEN` | Una clave local que habilita las órdenes simuladas desde la pantalla. La pantalla la manda sola: no se escribe en ningún lado más. | La pantalla funciona, pero las órdenes del Terminal dan error 403 (ver abajo). |
| `TELEGRAM_BOT_TOKEN` y `TELEGRAM_CHAT_ID` | Mandan las alertas (kill switch, errores, desconexiones) a un chat de Telegram. | No hay alertas por Telegram; quedan solo en los eventos del Panel. |
| `DATABENTO_API_KEY` | Bajar historia de futuros de Databento (cuesta según lo que se baje). | No se puede bajar historia nueva de futuros. Lo ya bajado sigue sirviendo. |

Después de cambiar el `.env`, apagá con `Ctrl+C` y volvé a correr `infra/scripts/ui_liquidez.sh`.

---

## 5. Si algo no funciona

**"Sin conexión con el gateway"** (o los cuadros quedan en "Pidiendo los datos al motor…")
- El gateway está apagado o se cortó. Mirá la ventana de la Terminal donde corrías el script: si terminó, volvé a correr `infra/scripts/ui_liquidez.sh`.
- Si el script dice "el gateway no arrancó (¿PostgreSQL está levantado?)", la base está caída (ver más abajo).

**Una orden da "403"** ("escritura apagada: falta el token local del gateway" o "falta la cabecera x-cfds-token")
- Falta `CFDS_GATEWAY_TOKEN` en el `.env`, o el script se prendió antes de agregarlo.
- Agregalo, apagá con `Ctrl+C` y volvé a correr el script. Mirar todo lo demás funciona igual sin el token.

**Error "relation … does not exist"** (por ejemplo, `learning_reports`)
- La base no tiene una tabla nueva: falta aplicar las migraciones del código nuevo.
- Apagá con `Ctrl+C`, corré `cd engine && cargo run -q -p cfds-storage --example migrate` y volvé a prender con el script.

**La base de datos está caída**
- Abrí Docker Desktop y fijate que esté corriendo.
- En esta Mac, el comando `docker` está en `~/.docker/bin/docker` y no siempre se encuentra solo. Si `docker` dice "command not found", usá la ruta completa:
  ```text
  ~/.docker/bin/docker compose -f infra/docker-compose.yml --env-file .env up -d
  ~/.docker/bin/docker compose -f infra/docker-compose.yml ps
  ```
- Los dos servicios tienen que decir `healthy`. Después volvé a correr el script.

**El disco está lleno**
- Lo que más ocupa son las carpetas de compilación del motor: `engine/target`, y las de cada copia de trabajo de los agentes en `.claude/worktrees/*/engine/target`. Pueden pasar de 15 GB cada una.
- Se pueden borrar sin perder nada: se vuelven a generar en la próxima compilación, que va a tardar más.
  ```text
  rm -rf engine/target
  ```
- Para ver el espacio libre: `df -h ~`.

**La pantalla muestra "datos de ejemplo"**
- Esa parte todavía es una maqueta, o el motor no respondió. No es un error, pero esos números no son reales.
