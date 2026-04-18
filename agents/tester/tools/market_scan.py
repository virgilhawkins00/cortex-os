import os
import random

# Mock script for market scanning
# Arguments are passed via environment variables (e.g. ARG_SYMBOL)
symbol = os.getenv("ARG_SYMBOL", "BTC/USD")

print(f"--- Scanning Market for {symbol} ---")
rsi = random.uniform(30, 70)
price = random.uniform(40000, 60000)

print(f"Price: ${price:.2f}")
print(f"RSI: {rsi:.2f}")

if rsi < 40:
    print("STATUS: OVERSOLD - Potential Buy Signal")
elif rsi > 60:
    print("STATUS: OVERBOUGHT - Potential Sell Signal")
else:
    print("STATUS: NEUTRAL")

print("--- Scan Complete ---")
