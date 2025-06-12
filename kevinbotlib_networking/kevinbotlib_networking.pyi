from collections.abc import Callable


class NetworkServer:
    """
    KevinbotLib Socket Network Server
    """
    def __init__(self, host: str, port: int):
        ...

    def start(self) -> None:
        ...
    def stop(self) -> None:
        ...

    @property
    def running(self) -> bool:
        ...

    @logger.setter
    def logger(self, logger: Callable[[str, str], None]):
        ...

class BlockingClient:
    def __init__(self, host: str, port: int):
        ...

    @logger.setter
    def logger(self, logger: Callable[[str, str], None]):
        ...

    def connect(self) -> None:
        ...

    def close(self) -> None:
        ...

    def set(self, key: str, value: str) -> None:
        ...

    def get(self, key: str) -> str | None:
        ...

    def get_all_keys(self) -> list[str]:
        ...

    def get_key_count(self) -> int:
        ...

    def delete(self, key: str) -> None:
        ...

    def send_command(self, message: str) -> str:
        ...

    @property
    def connected(self) -> bool:
        ...