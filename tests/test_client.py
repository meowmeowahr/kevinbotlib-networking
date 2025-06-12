import time

from kevinbotlib_networking import NetworkServer, BlockingClient


def test_client():
    server = NetworkServer("127.0.0.1", 15387)
    server.start()
    time.sleep(0.5)
    assert server.running == True

    client = BlockingClient("127.0.0.1", 15387)
    assert client.connected == False
    client.connect()
    assert client.connected == True
    time.sleep(0.5)

    server.stop()
    time.sleep(0.5)
    client.set("x", "test")
    client.set("x", "test")
    client.set("x", "test")
    client.set("x", "test")
    client.set("x", "test")
    client.set("x", "test")
    client.set("x", "test")
