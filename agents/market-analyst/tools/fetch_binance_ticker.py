#!/usr/bin/env python3
import sys
import json
import urllib.request
import urllib.error

def fetch_ticker(symbol):
    url = f"https://api.binance.com/api/v3/ticker/price?symbol={symbol}"
    try:
        req = urllib.request.Request(url, headers={'User-Agent': 'Mozilla/5.0'})
        with urllib.request.urlopen(req) as response:
            data = json.loads(response.read().decode())
            print(f"--- Ticker for {symbol} ---")
            print(f"Price: ${data.get('price')}")
            print("STATUS: SUCCESS")
    except urllib.error.URLError as e:
        print(f"Failed to fetch data for {symbol}: {e}")

if __name__ == "__main__":
    if len(sys.argv) < 2:
        print("Usage: sc_fetch_binance_ticker <SYMBOL> (e.g., BTCUSDT)")
        sys.exit(1)
    
    symbol = sys.argv[1].upper()
    fetch_ticker(symbol)
