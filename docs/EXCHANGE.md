# Bybit Testnet Exchange Configuration

This document describes the instrument types available on Bybit testnet and how they are mapped in our trading system.

## Overview

Bybit Testnet provides a sandbox environment for testing trading strategies without risking real funds. The testnet supports multiple instrument types with varying levels of availability.

## Testnet Base URLs

- **REST API**: `https://api-testnet.bybit.com`
- **WebSocket Public (Spot)**: `wss://stream-testnet.bybit.com/v5/public/spot`
- **WebSocket Public (Linear/Perp)**: `wss://stream-testnet.bybit.com/v5/public/linear`
- **WebSocket Public (Option)**: `wss://stream-testnet.bybit.com/v5/public/option`
- **WebSocket Private**: `wss://stream-testnet.bybit.com/v5/private`

## Supported Instrument Types

### 1. Spot Trading

**Availability**: ✅ Live on Testnet

**Primary Symbol**: `USDC/USDT`

**Bybit API Format**: `USDCUSDT`

**Characteristics**:
- Physical delivery (no leverage)
- Maker/taker fee structure
- Good liquidity on testnet
- Real-time order book depth
- Suitable for low-latency microstructure strategies

**WebSocket Streams**:
- Order Book L2: `orderbook.50.USDCUSDT`
- Public Trades: `publicTrade.USDCUSDT`
- Tickers: `tickers.USDCUSDT`

**REST Endpoints**:
- Place Order: `POST /v5/order/create`
- Cancel Order: `POST /v5/order/cancel`
- Get Open Orders: `GET /v5/order/realtime`
- Get Order History: `GET /v5/order/history`

### 2. Perpetual Contracts (Linear)

**Availability**: ✅ Live on Testnet

**Primary Symbol**: `BTCUSDT` (perpetual)

**Bybit API Format**: `BTCUSDT` (category: `linear`)

**Characteristics**:
- USDT-margined perpetual contracts
- Leverage up to 100x (configurable)
- Funding rate mechanism
- Mark price and index price
- Higher liquidity than spot typically
- Good for testing leverage strategies

**WebSocket Streams**:
- Order Book L2: `orderbook.50.BTCUSDT`
- Public Trades: `publicTrade.BTCUSDT`
- Tickers: `tickers.BTCUSDT`

**Additional Considerations**:
- Monitor funding rates (every 8 hours)
- Position management required
- Liquidation price tracking
- Leverage impacts margin requirements

### 3. Options

**Availability**: ⚠️ Limited / Variable on Testnet

**Status**: **Options availability on Bybit testnet can vary**. The production environment has a robust options platform, but testnet options may have:
- Limited symbol availability
- Reduced liquidity
- Intermittent availability
- Fewer expiration dates

**Approach for this System**:
1. **Check availability** via API before going live
2. If available: use live mode with options symbols (e.g., `BTC-4MAR26-50000-C`)
3. If NOT available: use **replay mode** with historical options data

**Bybit Options Format** (when available):
- Format: `{underlying}-{expiry}-{strike}-{type}`
- Example: `BTC-28FEB26-50000-C` (BTC Call, strike 50000, expires Feb 28, 2026)
- Category: `option`

**WebSocket Streams** (if available):
- Order Book: `orderbook.50.BTC-28FEB26-50000-C`
- Public Trades: `publicTrade.BTC-28FEB26-50000-C`
- Tickers: `tickers.BTC-28FEB26-50000-C`

**Replay Mode**:
When options are not available on testnet, the system will:
- Load historical market data from JSONL files
- Simulate order execution with realistic fills
- Generate full metrics and logs as if trading live
- Allow testing of options strategies without live testnet access

## Symbol Mapping in Code

Our system uses a unified symbol format internally and maps to Bybit's format:

| Internal Format | Bybit Format | Category | Instrument Type |
|----------------|--------------|----------|-----------------|
| `USDC/USDT`    | `USDCUSDT`   | `spot`   | Spot |
| `BTC/USDT:PERP`| `BTCUSDT`    | `linear` | Perpetual |
| `ETH/USDT:PERP`| `ETHUSDT`    | `linear` | Perpetual |
| `BTC-28FEB26-50000-C` | `BTC-28FEB26-50000-C` | `option` | Call Option |
| `BTC-28FEB26-50000-P` | `BTC-28FEB26-50000-P` | `option` | Put Option |

## Account Setup

To use Bybit testnet:

1. **Create Account**: Visit https://testnet.bybit.com
2. **Get Test Funds**: Use the testnet faucet to get USDT/USDC
3. **Generate API Keys**:
   - Go to API Management
   - Create new API key
   - Enable required permissions:
     - Read-Write for Orders
     - Read for Account/Position
     - Read for Market Data
4. **Configure System**: Copy API key and secret to `.env` file

## Data Quality Notes

**Spot (USDC/USDT)**:
- ✅ Good depth and spread characteristics
- ✅ Frequent trades
- ✅ Low latency WebSocket feeds
- ⚠️ Testnet can have wider spreads than production

**Perpetuals (BTC/USDT)**:
- ✅ Excellent liquidity
- ✅ Tight spreads
- ✅ High message rate
- ✅ Representative of production conditions

**Options**:
- ⚠️ May have limited activity on testnet
- ⚠️ Wider spreads
- ⚠️ Less frequent trades
- 💡 Recommended to use replay mode for consistent testing

## API Rate Limits

Bybit testnet has rate limits similar to production:

- **REST API**: 
  - 120 requests per minute per IP (reading)
  - 60 requests per minute per IP (trading)
  
- **WebSocket**:
  - 500 subscriptions per connection
  - 10 connections per IP

## Testing Strategy

1. **Start with Spot**: Test basic market data and execution with `USDC/USDT`
2. **Add Perpetuals**: Test leverage and position management with `BTCUSDT`
3. **Options (if available)**: Test with live options or use replay mode
4. **Multi-instrument**: Run strategies across all three simultaneously

## Monitoring & Validation

Before going live on any instrument:
- ✅ Verify WebSocket connectivity
- ✅ Confirm order book updates are flowing
- ✅ Test small order placement and cancellation
- ✅ Verify fill reporting
- ✅ Check account balance updates

## References

- [Bybit API Documentation](https://bybit-exchange.github.io/docs/v5/intro)
- [Bybit Testnet](https://testnet.bybit.com)
- [WebSocket API](https://bybit-exchange.github.io/docs/v5/ws/connect)
- [REST API](https://bybit-exchange.github.io/docs/v5/order/create-order)
