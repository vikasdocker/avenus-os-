from pathlib import Path
import tempfile
import unittest

from aether_brain import ConversationEngine, Intent, IntentClassifier
from aether_sdk import Response, parse_command


class IntentClassifierTests(unittest.TestCase):
    def test_status_intent(self) -> None:
        result = IntentClassifier().classify("system status")
        self.assertEqual(result.intent, Intent.STATUS)
        self.assertGreater(result.confidence, 0.9)

    def test_open_application_intent(self) -> None:
        result = IntentClassifier().classify("open terminal")
        self.assertEqual(result.intent, Intent.OPEN_APPLICATION)
        self.assertEqual(result.subject, "terminal")


class ConversationEngineTests(unittest.TestCase):
    def test_status_response(self) -> None:
        result = ConversationEngine().handle("status")
        self.assertEqual(result.action, "report_status")
        self.assertIn("ready", result.response)

    def test_memory_is_written(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            memory_path = Path(directory) / "memory.json"
            result = ConversationEngine(memory_path=memory_path).handle("search boot logs")
            self.assertEqual(result.action, "search_files:boot logs")
            self.assertTrue(memory_path.exists())
            self.assertIn("last_intent", memory_path.read_text(encoding="utf-8"))


class PythonSdkTests(unittest.TestCase):
    def test_parse_command(self) -> None:
        self.assertEqual(parse_command("open terminal"), ("open", "terminal"))

    def test_response_serialization(self) -> None:
        response = Response(status="ok", message="line one\nline two")
        self.assertEqual(response.serialize(), "AETHER/1 status=ok message=line one\\nline two")


if __name__ == "__main__":
    unittest.main()

