import time

from kevinbotlib_networking import NetworkServer

def log(level: str, message: str):
    print(level, message)

srv = NetworkServer("127.0.0.1", 8888)
srv.logger = log
srv.start()

print("Server running:", srv.running)

time.sleep(20)

srv.stop()