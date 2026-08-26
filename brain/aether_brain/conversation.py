from dataclasses import dataclass
from pathlib import Path

from .intent import Intent, IntentClassifier, IntentResult
from .memory import MemoryStore


@dataclass(frozen=True)
class ConversationResult:
    intent: str
    confidence: float
    response: str
    action: str


class ConversationEngine:
    def __init__(self, memory_path: Path | None = None):
        self._classifier = IntentClassifier()
        self._memory = MemoryStore(memory_path) if memory_path is not None else None

    def handle(self, text: str) -> ConversationResult:
        result = self._classifier.classify(text)
        if self._memory is not None and result.intent is not Intent.UNKNOWN:
            self._memory.put("last_intent", result.intent.value)
            self._memory.put("last_subject", result.subject)
        return self._respond(result)

    def _respond(self, result: IntentResult) -> ConversationResult:
        if result.intent is Intent.STATUS:
            return ConversationResult(
                intent=result.intent.value,
                confidence=result.confidence,
                response="Aether core services are ready for local control.",
                action="report_status",
            )

        if result.intent is Intent.OPEN_APPLICATION:
            return ConversationResult(
                intent=result.intent.value,
                confidence=result.confidence,
                response=f"I can route an application launch request for '{result.subject}'.",
                action=f"open_application:{result.subject}",
            )

        if result.intent is Intent.SEARCH_FILES:
            return ConversationResult(
                intent=result.intent.value,
                confidence=result.confidence,
                response=f"I can start a local file search for '{result.subject}'.",
                action=f"search_files:{result.subject}",
            )

        if result.intent is Intent.HELP:
            return ConversationResult(
                intent=result.intent.value,
                confidence=result.confidence,
                response="Available local commands: status, open application, search files, help.",
                action="show_help",
            )

        return ConversationResult(
            intent=result.intent.value,
            confidence=result.confidence,
            response="I need a clearer local system request before acting.",
            action="ask_clarification",
        )

