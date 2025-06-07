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