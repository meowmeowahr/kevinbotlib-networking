from kevinbotlib_networking import BlockingClient

if __name__ == '__main__':
    client = BlockingClient("127.0.0.1", 8888)
    client.connect()

    while True:
        data = input(">>> ")
        if not data:
            continue
        
        print("<<<", client.send_command(data))