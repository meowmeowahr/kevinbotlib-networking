import time

from kevinbotlib_networking.kevinbotlib_networking import NetworkServer


def test_server_startup_shutdown():
    server = NetworkServer("127.0.0.1", 15387)
    server.start()
    time.sleep(0.5)
    assert server.running == True
    server.stop()
    time.sleep(0.5)
    assert server.running == False