#!/usr/bin/env python3

import argparse
import json
import time


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--url")
    parser.add_argument("--labels")
    parser.add_argument("--cookies")
    parser.add_argument("--token")
    parser.add_argument("--gemini-api-key", dest="gemini_api_key")
    parser.add_argument("--user-agent", dest="user_agent")
    parser.add_argument("--headless", action="store_true")
    parser.add_argument("--no-headless", dest="headless", action="store_false")
    parser.add_argument("--sleep", type=float, default=60.0)
    _ = parser.parse_args()

    time.sleep(_.sleep)
    print(json.dumps([], ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
