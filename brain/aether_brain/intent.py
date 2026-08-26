from dataclasses import dataclass
from enum import Enum


class Intent(str, Enum):
    STATUS = "system.status"
    OPEN_APPLICATION = "application.open"
    SEARCH_FILES = "files.search"
    HELP = "assistant.help"
    UNKNOWN = "unknown"


@dataclass(frozen=True)
class IntentResult:
    intent: Intent
    confidence: float
    subject: str


class IntentClassifier:
    def classify(self, text: str) -> IntentResult:
        normalized = " ".join(text.strip().lower().split())
        if not normalized:
            return IntentResult(Intent.UNKNOWN, 0.0, "")

        if normalized in {"status", "system status", "health", "are you ready"}:
            return IntentResult(Intent.STATUS, 0.98, "system")

        for prefix in ("open ", "launch ", "start "):
            if normalized.startswith(prefix):
                subject = normalized.removeprefix(prefix).strip()
                if subject:
                    return IntentResult(Intent.OPEN_APPLICATION, 0.90, subject)

        for prefix in ("find ", "search ", "locate "):
            if normalized.startswith(prefix):
                subject = normalized.removeprefix(prefix).strip()
                if subject:
                    return IntentResult(Intent.SEARCH_FILES, 0.86, subject)

        if normalized in {"help", "what can you do", "commands"}:
            return IntentResult(Intent.HELP, 0.95, "help")

        return IntentResult(Intent.UNKNOWN, 0.20, normalized)

