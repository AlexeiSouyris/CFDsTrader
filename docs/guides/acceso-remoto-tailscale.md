# Entrar a la terminal desde otra compu (Tailscale)

La Mac mini hace de servidor: los backtests corren ahí, y la MacBook o el celu solo muestran la pantalla.

## Cómo está armado (#859)

- **Tailscale** arma una red privada entre los aparatos del dueño (misma cuenta). Nada queda abierto a internet ni al wifi donde estés.
- `infra/scripts/tailnet_forward.py` escucha **solo en la IP de Tailscale** de la Mac mini (100.x) y le pasa el tráfico a la terminal (Vite, `localhost:5173`). Desde el wifi de la casa o de la facu esa dirección no existe.
- La terminal sigue igual que en la Mac mini: el gateway queda en `127.0.0.1` y el token de escritura lo pone el servidor de Vite, nunca el navegador.

## Para entrar

1. En la MacBook (o el celu): instalá Tailscale e iniciá sesión con **la misma cuenta**.
2. Abrí `http://<IP de Tailscale de la Mac mini>:5173`. La IP sale con `tailscale ip -4` en la Mac mini, o en el ícono de Tailscale.

## En la Mac mini

- Tienen que estar andando: Docker, el gateway + Vite (`sh infra/scripts/ui_liquidez.sh`) y el reenvío (`python3 -I infra/scripts/tailnet_forward.py`).
- La Mac no se duerme (`pmset sleep 0`); la pantalla sí se apaga a los 10 min.
- Arranque automático al prender la Mac: ver abajo (#863).

## Arranque automático (#863)

Una sola vez, en la Mac mini:

```sh
sh infra/scripts/install_autostart.sh
```

Desde ahí, al iniciar sesión arrancan solos:

- **Docker Desktop** (ítem de inicio de sesión), que levanta PostgreSQL y Redis.
- **La terminal** (`com.cfdstrader.terminal`): espera a la base y corre `ui_liquidez.sh` sin abrir el navegador.
- **El link** (`com.cfdstrader.tailnet`): `tailnet_forward.py`. Si Tailscale todavía no tiene IP, reintenta cada 30 s.

Si alguno se cae, `launchd` lo vuelve a levantar. Los registros quedan en `logs/autostart_*.log`.
Para sacarlo: `sh infra/scripts/install_autostart.sh --remove`.

**Límite:** con FileVault prendido, después de un corte de luz la Mac pide la contraseña al arrancar.
Hasta que alguien la escriba, no arranca nada. Eso no se puede (ni se debe) saltear.
