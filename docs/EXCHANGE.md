# Bybit Testnet Instruments & Symbol Mapping

This project targets Bybit v5 testnet and keeps symbol mapping explicit
so we do not assume instruments that might not exist in testnet.

## Endpoints (testnet)

- REST: https://api-testnet.bybit.com
- WS public: wss://stream-testnet.bybit.com/v5/public
- WS private: wss://stream-testnet.bybit.com/v5/private

## Symbol formatting

Input symbols are expressed as `BASE/QUOTE` (e.g., `USDC/USDT`).
Bybit v5 uses a concatenated symbol string (e.g., `USDCUSDT`) and a
`category` parameter to specify the market type.

## Spot (category=spot)

Default mapping:

- `USDC/USDT` -> `USDCUSDT`

Confirm with:

```
GET /v5/market/instruments-info?category=spot&symbol=USDCUSDT
```

If the symbol is missing, fail fast and require an explicit override.

## Perpetuals (category=linear)

"Closest" means a linear (USDT-margined) perp with base `USDC` and quote
`USDT`.

Confirm with:

```
GET /v5/market/instruments-info?category=linear&baseCoin=USDC&quoteCoin=USDT
```

- If present: use the returned symbol (typically `USDCUSDT`).
- If absent: do not guess. Require an explicit perp symbol override or
  run in replay mode.

## Options (category=option)

Options availability on testnet can vary by product and environment.
Always check first:

```
GET /v5/market/instruments-info?category=option
```

- If no instruments are returned: `--instrument option` must run in
  replay mode.
- If instruments exist: use the exact symbol strings returned by the API
  (Bybit option symbols are formatted like `BTC-28MAR25-30000-C`).

## Rationale

Testnet inventories change over time. The checks above avoid hardcoding
nonexistent instruments while keeping spot and perp live when available.
