"""Wire protocol primitives shared by Aether OS text surfaces."""

from __future__ import annotations

PROTOCOL_VERSION = "AETHER/1"


def parse_command(raw: str) -> tuple[str, str]:
    """Split ``raw`` into ``(verb, argument)``.

    The verb is the first whitespace-separated token (lowercased); the
    argument is everything after it, whitespace-normalized.
    """
    tokens = raw.strip().split()
    if not tokens:
        return ("", "")
    verb = tokens[0].lower()
    argument = " ".join(tokens[1:])
    return (verb, argument)


class Response:
    """Serializable response line on the AETHER/1 wire protocol."""

    def __init__(self, status: str, message: str) -> None:
        self.status = status
        self.message = message

    def serialize(self) -> str:
        encoded = self.message.replace("\\", "\\\\").replace("\n", "\\n").replace("\r", "\\r")
        return f"{PROTOCOL_VERSION} status={self.status} message={encoded}"

    @classmethod
    def deserialize(cls, line: str) -> "Response":
        prefix = f"{PROTOCOL_VERSION} "
        if not line.startswith(prefix):
            raise ValueError(f"not an {PROTOCOL_VERSION} line: {line!r}")
        body = line[len(prefix):]
        fields: dict[str, str] = {}
        for chunk in body.split(" "):
            key, sep, value = chunk.partition("=")
            if not sep:
                continue
            fields[key] = value
        message = fields.get("message", "").replace("\\n", "\n").replace("\\r", "\r").replace("\\\\", "\\")
        return cls(status=fields.get("status", "error"), message=message)
