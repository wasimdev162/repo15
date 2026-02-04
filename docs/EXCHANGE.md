# Bybit Testnet Instruments + Symbol Mapping

This repo targets **Bybit v5** APIs and keeps a **canonical** symbol format for humans:

- Canonical: `BASE/QUOTE` (example: `USDC/USDT`)
- Bybit symbol: `BASEQUOTE` (example: `USDCUSDT`)

The runtime `--instrument` selects **which Bybit market category** (and WS endpoint) we use.

---

## Endpoints (Bybit v5)

### REST (testnet)

- `BYBIT_REST_BASE_URL`: `https://api-testnet.bybit.com`

### WebSocket public (testnet)

- **Spot**: `wss://stream-testnet.bybit.com/v5/public/spot`
- **Perp (linear)**: `wss://stream-testnet.bybit.com/v5/public/linear`
- **Options**: `wss://stream-testnet.bybit.com/v5/public/option`

### WebSocket private (testnet)

- `wss://stream-testnet.bybit.com/v5/private`

---

## Instrument types we support (and how we stay honest)

Bybit testnet availability can vary by product, especially **options**. We do **not** assume options are live on testnet.

- **Spot**: live (target symbol: `USDCUSDT`)
- **Perp**: live **if** the selected linear perpetual exists on testnet
- **Options**:
  - live **only if** testnet returns option instruments for the requested symbol(s)
  - otherwise run **replay mode** while still producing full logs/metrics

---

## Symbol mapping

### Canonical → Bybit

- `USDC/USDT` → `USDCUSDT`

General rule:

- Remove the slash: `BASE/QUOTE` → `BASEQUOTE`

Notes:

- Bybit symbol strings are case-sensitive in some contexts; use uppercase by default.
- For options, Bybit symbols are often **contract-coded** (not just `BASEQUOTE`). Treat options symbols as **discovered**, not derived.

---

## How to confirm what testnet actually supports (no guessing)

Use the v5 instruments endpoint to check availability.

### Spot check (example)

- `GET /v5/market/instruments-info?category=spot&symbol=USDCUSDT`

### Linear perp check (example)

- `GET /v5/market/instruments-info?category=linear&symbol=<CANDIDATE>`

If `USDCUSDT` is not available as a linear perp on testnet, pick the closest supported perp by listing instruments:

- `GET /v5/market/instruments-info?category=linear`

Then choose an active USDT-settled perpetual (commonly things like `BTCUSDT`, `ETHUSDT`, etc.) **based on what testnet returns**.

### Options availability check

Options symbols are not safely derivable from `BASE/QUOTE`. First list what exists:

- `GET /v5/market/instruments-info?category=option`

If this returns no instruments (or none you can trade on testnet), treat options as **replay-only**.

---

## Mapping from `--instrument` to Bybit v5 categories

- `--instrument spot` → REST category `spot`, WS public endpoint `/v5/public/spot`
- `--instrument perp` → REST category `linear`, WS public endpoint `/v5/public/linear`
- `--instrument option` → REST category `option`, WS public endpoint `/v5/public/option` (live only if available; else replay)

---

## Environment variables we rely on

See `.env.example` for the exact names.

- `BYBIT_API_KEY`, `BYBIT_API_SECRET`
- `BYBIT_TESTNET=true`
- `BYBIT_REST_BASE_URL`
- `BYBIT_WS_PUBLIC_{SPOT,LINEAR,OPTION}_URL`
- `BYBIT_WS_PRIVATE_URL`
- `BYBIT_SPOT_SYMBOL` (defaults to `USDCUSDT`)
- `BYBIT_LINEAR_SYMBOL`, `BYBIT_OPTION_SYMBOL` (must be set after discovery)

