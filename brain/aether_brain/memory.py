from dataclasses import asdict, dataclass
from datetime import UTC, datetime
import json
from pathlib import Path
from typing import Iterable


@dataclass(frozen=True)
class MemoryRecord:
    key: str
    value: str
    created_at: str

    @staticmethod
    def create(key: str, value: str) -> "MemoryRecord":
        timestamp = datetime.now(UTC).replace(microsecond=0).isoformat()
        return MemoryRecord(key=key, value=value, created_at=timestamp)


class MemoryStore:
    def __init__(self, path: Path):
        self._path = path

    def put(self, key: str, value: str) -> MemoryRecord:
        record = MemoryRecord.create(key, value)
        records = {existing.key: existing for existing in self.all()}
        records[key] = record
        self._write(records.values())
        return record

    def get(self, key: str) -> MemoryRecord | None:
        for record in self.all():
            if record.key == key:
                return record
        return None

    def all(self) -> list[MemoryRecord]:
        if not self._path.exists():
            return []
        with self._path.open("r", encoding="utf-8") as handle:
            payload = json.load(handle)
        return [MemoryRecord(**item) for item in payload.get("records", [])]

    def _write(self, records: Iterable[MemoryRecord]) -> None:
        self._path.parent.mkdir(parents=True, exist_ok=True)
        payload = {"records": [asdict(record) for record in sorted(records, key=lambda item: item.key)]}
        with self._path.open("w", encoding="utf-8") as handle:
            json.dump(payload, handle, indent=2, sort_keys=True)
            handle.write("\n")

