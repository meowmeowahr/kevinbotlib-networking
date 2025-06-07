import time

from kevinbotlib_networking import NetworkServer

def log(level: str, message: str):
    print(level, message)

srv = NetworkServer("127.0.0.1", 8888)
srv.logger = log

try:
    srv.start()
    while True:
        time.sleep(1)
finally:
    srv.stop()