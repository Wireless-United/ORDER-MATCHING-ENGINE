#!/usr/bin/env python3
"""
Small helper to send a FIX NewOrderSingle (35=D) message to a TCP FIX listener.
Defaults to host 127.0.0.1 port 9878 and a sample NewOrderSingle message.
Accepts optional command line args: host port
"""
import sys
import socket

HOST = sys.argv[1] if len(sys.argv) > 1 else '127.0.0.1'
PORT = int(sys.argv[2]) if len(sys.argv) > 2 else 9878

# Example pipe-separated FIX message (listener accepts '|' or SOH)
FIX_MSG = '8=FIX.4.2|9=123|35=D|54=1|44=100.5|38=10|55=Pranesh|10=000|\n'

if __name__ == '__main__':
    print(f"Sending FIX message to {HOST}:{PORT}...")
    with socket.create_connection((HOST, PORT), timeout=5) as s:
        s.sendall(FIX_MSG.encode('utf-8'))
    print("Message sent.")
