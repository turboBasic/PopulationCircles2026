import json
import os
import subprocess
import sys
from pathlib import Path

from repo_tools.redact import scrub

ANSWER_FILE = "agent-answer.md"


def opencode(*args: str) -> str | None:
    result = subprocess.run(  # noqa: S603 — argv, not a shell
        ["opencode", *args],  # noqa: S607 — trusted; pinned by mise
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode != 0:
        print(result.stderr.strip(), file=sys.stderr)
        return None
    return result.stdout


# `run` never prints the session id it used, in either output format, so it is read back from
# `session list` — sound only because the workflow step calling this runs exactly one session.
def latest_session_id() -> str | None:
    listing = opencode("session", "list", "--format", "json", "--max-count", "1")
    if not listing:
        return None
    sessions = json.loads(listing)
    return sessions[0]["id"] if sessions else None


# Read back from `export` rather than the run's own stdout: that stream is the pretty terminal
# rendering — tool calls and prose interleaved with ANSI escapes — and has no reliable seam to cut
# the final answer from once those are stripped. `export` gives the same session as structured JSON,
# with no second model call.
#
# Written to a file rather than captured through a pipe: opencode's own CLI entry point calls
# `process.exit()` in a `finally` block right after every command returns, with no wait for stdout
# to drain — a Node write to a pipe is non-blocking, so a large export can still be sitting in a
# buffer when that fires, truncating the JSON. Measured on a real session, not the small ones this
# step was tested against: `Unterminated string`. A file's write is synchronous, so the same race
# does not apply to it.
def final_answer(session_id: str) -> str:
    export_path = Path(os.environ["RUNNER_TEMP"], "session-export.json")
    with export_path.open("w") as handle:
        result = subprocess.run(  # noqa: S603 — argv, not a shell
            ["opencode", "export", session_id],  # noqa: S607 — trusted; pinned by mise
            stdout=handle,
            stderr=subprocess.PIPE,
            text=True,
            check=False,
        )
    if result.returncode != 0:
        print(result.stderr.strip(), file=sys.stderr)
        return ""
    # No answer beats a crashed step: this step's only job is a nice-to-have report, and a run that
    # produced a diff has a pull request to carry the result whether or not this parses.
    try:
        data = json.loads(export_path.read_text())
        assistant = [m for m in data["messages"] if m["info"]["role"] == "assistant"]
        if not assistant:
            return ""
        parts = assistant[-1]["parts"]
        return "".join(part["text"] for part in parts if part["type"] == "text")
    except (json.JSONDecodeError, LookupError) as error:
        print(f"could not read the exported session back: {error}", file=sys.stderr)
        return ""


def main() -> int:
    session_id = latest_session_id()
    if session_id is None:
        print("no opencode session found to read the answer from", file=sys.stderr)
        return 0
    answer = scrub(final_answer(session_id).strip())
    if answer:
        Path(os.environ["RUNNER_TEMP"], ANSWER_FILE).write_text(answer)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
