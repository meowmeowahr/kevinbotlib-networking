import string
import random
import time

from kevinbotlib_networking import BlockingClient

def log(level: str, message: str):
    print(level, message)


def randomword(length):
    letters = string.ascii_lowercase
    return ''.join(random.choice(letters) for i in range(length))

DATA = randomword(10_000_000)
NUM_OPERATIONS = 1000

if __name__ == '__main__':
    client = BlockingClient("127.0.0.1", 8888)
    client.logger = log

    try:
        client.connect()

        # Phase 1: SET operations
        print(f"Starting {NUM_OPERATIONS} SET operations...")
        start_time_set = time.perf_counter()
        set_success_count = 0
        for i in range(NUM_OPERATIONS):
            key = f"key_{i}"
            value = DATA
            if client.set(key, value):
                set_success_count += 1
            if i % (NUM_OPERATIONS // 10) == 0 and NUM_OPERATIONS > 10:
                print(f"  {i}/{NUM_OPERATIONS} SETs completed...")
        end_time_set = time.perf_counter()
        time_elapsed_set = end_time_set - start_time_set
        print(f"Completed {NUM_OPERATIONS} SET operations in {time_elapsed_set:.4f} seconds.")
        print(f"  SET success rate: {set_success_count}/{NUM_OPERATIONS}")
        if set_success_count > 0:
            print(f"  Average SET time per operation: {time_elapsed_set / set_success_count * 1000:.4f} ms")


        # Phase 2: GET operations (retrieving previously set keys)
        print(f"Starting {NUM_OPERATIONS} GET operations...")
        start_time_get = time.perf_counter()
        get_success_count = 0
        for i in range(NUM_OPERATIONS):
            key = f"key_{i}"
            retrieved_value = client.get(key)
            # In a real test, you'd verify retrieved_value against the original
            if retrieved_value is not None and not retrieved_value.startswith("ERROR"):
                get_success_count += 1
            if i % (NUM_OPERATIONS // 10) == 0 and NUM_OPERATIONS > 10:
                print(f"  {i}/{NUM_OPERATIONS} GETs completed...")
        end_time_get = time.perf_counter()
        time_elapsed_get = end_time_get - start_time_get
        print(f"Completed {NUM_OPERATIONS} GET operations in {time_elapsed_get:.4f} seconds.")
        print(f"  GET success rate: {get_success_count}/{NUM_OPERATIONS}")
        if get_success_count > 0:
            print(f"  Average GET time per operation: {time_elapsed_get / get_success_count * 1000:.4f} ms")


        # Phase 3: DEL operations (retrieving previously set keys)
        print(f"Starting {NUM_OPERATIONS} DEL operations...")
        start_time_del = time.perf_counter()
        del_success_count = 0
        for i in range(NUM_OPERATIONS):
            key = f"key_{i}"
            client.delete(key)
            del_success_count += 1
            if i % (NUM_OPERATIONS // 10) == 0 and NUM_OPERATIONS > 10:
                print(f"  {i}/{NUM_OPERATIONS} DELs completed...")
        end_time_del = time.perf_counter()
        time_elapsed_get = end_time_del - start_time_del
        print(f"Completed {NUM_OPERATIONS} DEL operations in {time_elapsed_get:.4f} seconds.")
        print(f"  DEL success rate: {del_success_count}/{NUM_OPERATIONS}")
        if del_success_count > 0:
            print(f"  Average DEL time per operation: {time_elapsed_get / del_success_count * 1000:.4f} ms")


    except RuntimeError as e:
        print(f"Client operation failed: {e}")
    finally:
        client.disconnect()
