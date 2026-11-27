# Consultas a proveedores (borradores)

Borradores para que el **dueño** los mande a mano (#388). **Ningún agente los envía.** Cada uno está en español y en inglés: mandá el idioma que prefieras.

Reglas para usarlos:
- No tienen datos personales, números de cuenta ni direcciones de correo del dueño. Agregá lo que haga falta (por ejemplo el usuario de la cuenta demo) **recién al enviar** y nunca lo commitees.
- Solo se indica un canal de contacto si aparece en una página oficial, con la URL y la fecha de lectura. Si no se pudo verificar, se dice.
- Cuando llegue la respuesta, anotala en el Issue que corresponda (citando las palabras del proveedor y la fecha) antes de cambiar la configuración.

| # | Proveedor | Tema | Canal oficial (leído el 24-11-2026) |
|---|---|---|---|
| 1 | IG (soporte de la API) | Habilitar Lightstreamer (`CHART` y `PRICE`) en la cuenta demo | `webapisupport@ig.com`, según https://labs.ig.com/faq.html |
| 2 | IG España | Tasa de referencia y tasa administrativa de la financiación de CFD de índices de EE. UU. en una cuenta en EUR | `atencionalcliente@ig.com`, chat de la web o WhatsApp, según https://www.ig.com/es/contactenos |
| 3 | Interactive Brokers | CFD de índices para residentes en España por API, costo de datos CME y Eurex, cuenta paper | No verificado: el sitio de IBKR rechazó la lectura automática. Usá el formulario de consulta para quien no es cliente que figura en la página de atención al cliente de su web. |
| 4 | Databento | Precio actual del plan Standard (uso personal, CME y Eurex en vivo) | `support@databento.com`, chat o Slack, según https://databento.com/support |
| 5 | IG España | CFD de futuros para el programa 3 (#513): mercados, spreads, contratos, mínimos, vencimientos y conversión de divisa | `atencionalcliente@ig.com`, chat de la web o WhatsApp, según https://www.ig.com/es/contactenos (como la consulta 2) |

---

## 1. IG: soporte de la API (streaming en la cuenta demo)

**Antecedentes:** `config/ig.toml` y #68 registran que, con la cuenta demo de la API, la suscripción `CHART` responde "Insufficient permissions" y `PRICE` responde "Invalid group" (medido el 20 y el 22-11-2026). Sin streaming, las velas solo se consiguen por REST y consumen el cupo semanal de históricos.

**Canal:** la FAQ de IG Labs (https://labs.ig.com/faq.html) indica escribir al soporte de la Web API en `webapisupport@ig.com`.

### Español

> **Asunto:** Habilitar la Streaming API (Lightstreamer) en una cuenta demo de la API
>
> Hola:
>
> Uso la REST API de IG con una cuenta demo (entorno DEMO) y una clave de API válida. El login y los endpoints REST funcionan bien, pero las suscripciones de la Streaming API (Lightstreamer) fallan:
>
> - `CHART:{epic}:{escala}` responde "Insufficient permissions".
> - `PRICE:{cuenta}:{epic}` responde "Invalid group".
>
> Lo probé varias veces entre el 20 y el 22 de noviembre de 2026, con la sesión recién creada (CST y X-SECURITY-TOKEN válidos) y el endpoint de Lightstreamer que devuelve el login.
>
> Mis preguntas:
> 1. ¿Hay que habilitar algún permiso para que una cuenta demo de la API pueda suscribirse a `CHART` y `PRICE`? Si es así, ¿pueden habilitarlo?
> 2. Si las cuentas demo no tienen acceso a esas suscripciones, ¿está documentado en algún sitio? ¿Qué cuenta o condición se necesita?
> 3. ¿Las suscripciones `PRICE` necesitan algún adaptador de datos o formato de grupo concreto?
>
> Puedo enviarles el identificador de la cuenta demo y el ID de la clave de API por un canal privado si los necesitan.
>
> Gracias.

### English

> **Subject:** Enabling the Streaming API (Lightstreamer) on an API demo account
>
> Hello,
>
> I use the IG REST API with a demo account (DEMO environment) and a valid API key. Login and the REST endpoints work fine, but Streaming API (Lightstreamer) subscriptions fail:
>
> - `CHART:{epic}:{scale}` returns "Insufficient permissions".
> - `PRICE:{account}:{epic}` returns "Invalid group".
>
> I tried several times between 20 and 22 November 2026, with a freshly created session (valid CST and X-SECURITY-TOKEN) and the Lightstreamer endpoint returned by the login.
>
> My questions:
> 1. Is there a permission that must be enabled for an API demo account to subscribe to `CHART` and `PRICE`? If so, could you enable it?
> 2. If demo accounts cannot use these subscriptions, is this documented anywhere? Which account type or condition is required?
> 3. Do `PRICE` subscriptions need a specific data adapter or item format?
>
> I can send the demo account identifier and the API key ID through a private channel if you need them.
>
> Thank you.

---

## 2. IG España: financiación de CFD de índices de EE. UU. en una cuenta en EUR

**Antecedentes (#379, leído el 24-11-2026):**
- https://www.ig.com/es/nuestros-costes, pestaña *CFD > Índices*: «Nuestra tasa administrativa es de un 3 %. Tanto para los contratos mini como para los estándar.»
- La misma página, pestaña *Barrera > Índices*: «Nuestra tasa administrativa es de un 2,5 % para los contratos de CFD estándar y de un 3 % para contratos minis.»
- Tasa de referencia, misma página: «Si vas largo, pagarás según el ESTR (o el tipo de interés de referencia correspondiente)». No dice cuál corresponde a US 500, Wall Street o US Tech 100.
- El KID público del CFD sobre índice en español (https://www.ig.com/usermanagement/kid/document/NON_CONTROLLED_RISK/IGCFD/igintles/es_ES/INDICES) es de julio de 2018 y no da porcentajes.
- `config/instruments.toml` usa 2,5 % para US100, US500, US30 y DE40, y deja pendiente la tasa de referencia de los índices de EE. UU.

**Canal:** https://www.ig.com/es/contactenos indica `atencionalcliente@ig.com`, el chat de la web y WhatsApp.

### Español

> **Asunto:** Financiación nocturna de CFD de índices de EE. UU. en una cuenta en EUR
>
> Hola:
>
> Tengo dudas sobre cómo se calcula la financiación nocturna de los CFD de índices al contado (sin vencimiento) en una cuenta con divisa base EUR, para estos mercados: US 500, Wall Street y US Tech 100, contratos en EUR (por ejemplo el de 1 € por punto). Para comparar, también Alemania 40.
>
> En https://www.ig.com/es/nuestros-costes (consultado el 24-11-2026) veo dos valores para los índices:
> - en la pestaña de CFD: «Nuestra tasa administrativa es de un 3 %. Tanto para los contratos mini como para los estándar.»
> - en la pestaña de barrera: «un 2,5 % para los contratos de CFD estándar y de un 3 % para contratos minis.»
>
> La página también dice que los largos pagan «según el ESTR (o el tipo de interés de referencia correspondiente)».
>
> Mis preguntas:
> 1. ¿Cuál es hoy la tasa administrativa anual para los CFD al contado de US 500, Wall Street, US Tech 100 y Alemania 40, en contratos estándar y en mini? ¿El 3 % reemplazó al 2,5 %? Si cambió, ¿desde qué fecha?
> 2. ¿Qué tasa de referencia se aplica a US 500, Wall Street y US Tech 100 cuando el contrato está en EUR: el €STR (por la divisa del contrato) o una tasa en USD como SOFR (por el subyacente)? ¿Y para Alemania 40?
> 3. ¿Se aplica algún mínimo o redondeo a la tasa de referencia (por ejemplo, cuando es negativa)?
> 4. ¿El divisor es 360 para todos estos mercados, y el cargo se hace a las 23:00 CET? ¿Los viernes se cobran tres noches?
> 5. ¿Hay un documento oficial y actualizado (KID u hoja de condiciones del mercado) con estos datos? El KID público que encontré es de julio de 2018.
>
> Gracias.

### English

> **Subject:** Overnight funding for US index CFDs on a EUR account
>
> Hello,
>
> I have questions about how overnight funding is calculated for undated (cash) index CFDs on an account with EUR as its base currency, for these markets: US 500, Wall Street and US Tech 100, EUR-denominated contracts (for example the €1 per point one). For comparison, also Germany 40.
>
> On https://www.ig.com/es/nuestros-costes (read on 24 November 2026) I see two values for indices:
> - on the CFD tab: "Nuestra tasa administrativa es de un 3 %. Tanto para los contratos mini como para los estándar." (an admin fee of 3% for both mini and standard contracts);
> - on the barrier tab: 2.5% for standard contracts and 3% for minis.
>
> The page also says long positions pay "según el ESTR (o el tipo de interés de referencia correspondiente)" (based on €STR or the relevant benchmark rate).
>
> My questions:
> 1. What is the current annual admin fee for undated CFDs on US 500, Wall Street, US Tech 100 and Germany 40, for standard and mini contracts? Did 3% replace 2.5%? If it changed, from which date?
> 2. Which benchmark rate applies to US 500, Wall Street and US Tech 100 when the contract is in EUR: €STR (contract currency) or a USD rate such as SOFR (underlying)? And for Germany 40?
> 3. Is there a floor or rounding applied to the benchmark rate (for example, when it is negative)?
> 4. Is the day-count divisor 360 for all of these markets, and is the charge made at 23:00 CET? Are three nights charged on Fridays?
> 5. Is there an official, up-to-date document (KID or market information sheet) with these figures? The public KID I found is dated July 2018.
>
> Thank you.

---

## 3. Interactive Brokers: CFD de índices, datos y cuenta paper

**Antecedentes:** IBKR es una alternativa evaluada para datos y ejecución. Ningún dato de esta sección se pudo leer de su web (rechazó la lectura automática el 24-11-2026), por eso todo va como pregunta.

**Canal:** no verificado. En la web de IBKR, la página de atención al cliente ofrece un formulario de consulta para quien todavía no es cliente.

### Español

> **Asunto:** Consulta previa a abrir cuenta: CFD de índices por API, datos de mercado y cuenta paper (residente en España)
>
> Hola:
>
> Soy residente en España y estoy evaluando abrir una cuenta individual para uso personal (no profesional). Quiero operar por API (TWS API o Client Portal API). Antes de abrirla tengo estas preguntas:
>
> 1. **CFD de índices:** ¿se ofrecen CFD sobre índices (por ejemplo, sobre el S&P 500, el Nasdaq 100, el Dow Jones y el DAX) a clientes minoristas residentes en España? ¿Se pueden consultar precios y enviar órdenes de esos CFD por la API?
> 2. **Datos de mercado en tiempo real:** ¿cuál es hoy el costo mensual, para un suscriptor **no profesional**, de los paquetes que incluyen los futuros de CME (CME, CBOT) y de Eurex? ¿Se requiere un saldo mínimo o un gasto mínimo en comisiones para mantener las suscripciones?
> 3. **Cuenta paper:** ¿se puede usar la cuenta paper (simulada) por API sin haber depositado fondos en la cuenta real? Si se necesita un depósito, ¿de cuánto? ¿La cuenta paper recibe los mismos datos de mercado que las suscripciones de la cuenta real?
> 4. **Tipo de cuenta:** ¿alcanza con una cuenta Cash (sin margen) para contratar las suscripciones de datos de mercado y usar la cuenta paper? ¿Los CFD exigen una cuenta de margen?
>
> Gracias.

### English

> **Subject:** Pre-account question: index CFDs via API, market data and paper account (Spain resident)
>
> Hello,
>
> I am a resident of Spain evaluating opening an individual account for personal (non-professional) use. I want to trade through the API (TWS API or Client Portal API). Before opening it I have these questions:
>
> 1. **Index CFDs:** are index CFDs (for example on the S&P 500, Nasdaq 100, Dow Jones and DAX) offered to retail clients resident in Spain? Can prices for those CFDs be requested and orders sent through the API?
> 2. **Real-time market data:** what is the current monthly cost, for a **non-professional** subscriber, of the packages that include CME futures (CME, CBOT) and Eurex? Is a minimum balance or minimum commission spend required to keep the subscriptions?
> 3. **Paper account:** can the paper (simulated) account be used through the API without depositing funds in the live account? If a deposit is needed, how much? Does the paper account receive the same market data as the live account's subscriptions?
> 4. **Account type:** is a Cash account (no margin) enough to subscribe to market data and use the paper account? Do CFDs require a margin account?
>
> Thank you.

---

## 4. Databento: precio del plan Standard

**Antecedentes (leído el 24-11-2026):** la documentación de Databento (https://databento.com/docs/portal/live-data) dice que los datos en tiempo real de CME y Eurex están incluidos «with Standard plan (up to 2 devices)» para uso personal. La página de precios (https://databento.com/pricing) no se pudo leer sin navegador, así que el precio actual no está confirmado.

**Canal:** https://databento.com/support indica `support@databento.com`, el chat de la web y su comunidad de Slack.

### Español

> **Asunto:** Precio actual del plan Standard para uso personal (CME y Eurex en vivo)
>
> Hola:
>
> Estoy evaluando Databento para un proyecto personal, no comercial, con datos de futuros de índices.
>
> 1. ¿Cuál es hoy el precio mensual del plan Standard para uso personal? ¿Hay precio anual o descuentos?
> 2. ¿El plan Standard incluye los datos en vivo de CME Globex (MDP 3.0) y de Eurex para uso personal sin tarifas de licencia adicionales del mercado? ¿Hay que firmar algún acuerdo con CME o Eurex como suscriptor no profesional?
> 3. ¿Qué datos históricos (esquemas OHLCV de 1 minuto y diarios, y desde qué año) incluye el plan sin costo adicional para esos dos mercados?
> 4. ¿Hay alguna restricción para guardar localmente los datos descargados y usarlos en backtests personales?
> 5. ¿Algún requisito o limitación para un usuario residente en España?
>
> Gracias.

### English

> **Subject:** Current Standard plan price for personal use (live CME and Eurex)
>
> Hello,
>
> I am evaluating Databento for a personal, non-commercial project using index futures data.
>
> 1. What is the current monthly price of the Standard plan for personal use? Is there annual pricing or a discount?
> 2. Does the Standard plan include live CME Globex (MDP 3.0) and Eurex data for personal use with no additional venue license fees? Do I need to sign any agreement with CME or Eurex as a non-professional subscriber?
> 3. Which historical data (1-minute and daily OHLCV schemas, and from which year) does the plan include at no extra cost for those two venues?
> 4. Are there any restrictions on storing downloaded data locally and using it for personal backtests?
> 5. Are there any requirements or limitations for a user resident in Spain?
>
> Thank you.

---

## 5. IG España: CFD de futuros para el programa 3 (#513)

**Antecedentes (leído el 24-11-2026, detalle en `docs/analysis/2026-11-programa-3-borrador.md` §C):**
- https://www.ig.com/es/nuestros-costes ofrece CFD de futuros «sobre índices y materias primas» y publica el spread mínimo del futuro solo de FTSE 100 (4), Wall Street (6), Alemania 40 (6), Australia 200 (3), oro (0,9), plata (5,5), crudo EE. UU. (6) y Brent (6). En la pregunta frecuente sobre futuros muestra además EUR/USD 10, GBP/USD 9, «Oro al contado 0,6» y «Plata al contado 3». Sobre la conversión dice «El coste estándar de conversión es de 0,8 puntos».
- https://www.ig.com/es/futuros publica el T-Bond decimalizado (spread 4, margen 20 %) y el oro con spread 0,6.
- https://www.ig.com/uk/futures-trading/futures-vs-forwards (IG Reino Unido) dice que los CFD de forex no tienen versión de futuros, y https://www.ig.com/uk/charges da la conversión como «0.8% fee».
- Las fichas de mercado de IG España (por ejemplo https://www.ig.com/es/bonos/mercado-bonos/bund) dan tamaño mínimo, tamaño de contrato, margen y el próximo vencimiento, pero no dicen si son del CFD de futuros o del de contado, ni si son del contrato estándar o del mini.

**Canal:** https://www.ig.com/es/contactenos indica `atencionalcliente@ig.com`, el chat de la web y WhatsApp.

### Español

> **Asunto:** CFD de futuros: mercados disponibles, spreads, vencimientos y conversión de divisa (cuenta en EUR)
>
> Hola:
>
> Tengo una cuenta de CFD en EUR con IG España y quiero conocer las condiciones de los CFD de futuros (con vencimiento, sin financiación nocturna) para estos mercados: US 500, EE. UU. Tech 100, Russell 2000, Japón 225, EU Stocks 50, T-Note a 10 años, T-Bond, Bund, EUR/USD, USD/JPY, GBP/USD, AUD/USD, oro, plata, crudo EE. UU. y cobre de alto grado.
>
> 1. ¿Cuáles de esos mercados ofrecen como **CFD de futuros** a clientes minoristas de IG España? En particular, ¿ofrecen CFD de futuros (forwards) de EUR/USD, USD/JPY, GBP/USD y AUD/USD? En su web del Reino Unido entiendo que no, pero en https://www.ig.com/es/nuestros-costes aparecen EUR/USD (10) y GBP/USD (9) en la tabla de futuros.
> 2. Para cada CFD de futuros disponible: el **spread** (y si varía por horario), el **tamaño de contrato**, el **tamaño mínimo** y el **margen minorista**, tanto del contrato estándar como del mini, si existe.
> 3. Para el oro, en una página figura un spread de futuros de 0,9 y en otra de 0,6. ¿Cuál es el vigente?
> 4. Los datos de las fichas de mercado (tamaño mínimo, tamaño de contrato, margen), ¿corresponden al CFD de futuros o al de contado?
> 5. **Vencimientos y renovaciones:** ¿tienen un calendario de vencimientos de cada CFD de futuros, idealmente histórico (desde 2011), o la regla que usan (por ejemplo, cuántos días hábiles antes del vencimiento de la bolsa)? ¿En qué zona horaria están las horas publicadas en las fichas?
> 6. ¿La renovación (*rollover*) es automática o hay que pedirla? ¿Se cobran el spread de cierre y el de apertura completos al renovar?
> 7. **Conversión de divisa:** la web dice «0,8 puntos». ¿Es un 0,8 % sobre el importe convertido? ¿Se aplica a las ganancias y también a las pérdidas, y a los costes (spread, comisiones) en otra divisa?
> 8. La ficha del Russell 2000 no aparece en la web de IG España. ¿Lo ofrecen como CFD de futuros?
>
> Gracias.

### English

> **Subject:** Futures CFDs: available markets, spreads, expiries and currency conversion (EUR account)
>
> Hello,
>
> I hold a EUR CFD account with IG Spain and would like the terms of your futures CFDs (dated, with no overnight funding) for these markets: US 500, US Tech 100, Russell 2000, Japan 225, EU Stocks 50, 10-year T-Note, T-Bond, Bund, EUR/USD, USD/JPY, GBP/USD, AUD/USD, gold, silver, US crude and high grade copper.
>
> 1. Which of these markets do you offer as **futures CFDs** to IG Spain retail clients? In particular, do you offer futures (forward) CFDs on EUR/USD, USD/JPY, GBP/USD and AUD/USD? Your UK website suggests not, but https://www.ig.com/es/nuestros-costes lists EUR/USD (10) and GBP/USD (9) in the futures table.
> 2. For each futures CFD available: the **spread** (and whether it changes by time of day), the **contract size**, the **minimum deal size** and the **retail margin**, for both the standard and the mini contract where one exists.
> 3. For gold, one page shows a futures spread of 0.9 and another 0.6. Which one is current?
> 4. Do the figures on the market pages (minimum size, contract size, margin) refer to the futures CFD or the spot CFD?
> 5. **Expiries and rollovers:** do you have an expiry calendar for each futures CFD, ideally historical (since 2011), or the rule you use (for example, how many business days before the exchange expiry)? Which time zone do the times on the market pages use?
> 6. Is rollover automatic or must it be requested? Are the full closing and opening spreads charged on rollover?
> 7. **Currency conversion:** the website says "0,8 puntos". Is that 0.8% of the converted amount? Does it apply to profits and to losses, and to costs (spread, commissions) in another currency?
> 8. The Russell 2000 market page does not appear on IG Spain's website. Do you offer it as a futures CFD?
>
> Thank you.
