from argparse import ArgumentParser
from dataclasses import asdict
import json
from pathlib import Path
import sys

from .conversation import ConversationEngine


def build_parser() -> ArgumentParser:
    parser = ArgumentParser(prog="aether-brain", description="Aether OS local brain interface")
    parser.add_argument("--once", metavar="TEXT", help="process one request and exit")
    parser.add_argument("--memory", type=Path, help="path to the local JSON memory store")
    parser.add_argument("--json", action="store_true", help="emit JSON responses")
    return parser


def emit(result, json_output: bool) -> None:
    if json_output:
        print(json.dumps(asdict(result), sort_keys=True))
    else:
        print(result.response)


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    engine = ConversationEngine(memory_path=args.memory)

    if args.once is not None:
        emit(engine.handle(args.once), args.json)
        return 0

    print("Aether brain ready. Type a request or 'exit'.", file=sys.stderr)
    for line in sys.stdin:
        request = line.strip()
        if request in {"exit", "quit"}:
            return 0
        if request:
            emit(engine.handle(request), args.json)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

