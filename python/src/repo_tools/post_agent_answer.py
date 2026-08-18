import json
import os
import subprocess
import sys


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


def gh(*args: str, stdin: str | None = None) -> str | None:
    result = subprocess.run(  # noqa: S603 — argv, not a shell
        ["gh", *args],  # noqa: S607 — trusted; the App token is read from GH_TOKEN
        input=stdin,
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
# with no second model call. No `--sanitize`: that flag redacts every text part wholesale — shares a
# session's shape, not its content — and would blank the very answer this posts.
def final_answer(session_id: str) -> str:
    exported = opencode("export", session_id)
    if not exported:
        return ""
    data = json.loads(exported)
    assistant = [m for m in data["messages"] if m["info"]["role"] == "assistant"]
    if not assistant:
        return ""
    parts = assistant[-1]["parts"]
    return "".join(part["text"] for part in parts if part["type"] == "text")


# The session held both secrets in its environment for the run that produced it, and export applies
# no scrubbing of its own — a targeted replace of the two known values, not opencode's `--sanitize`,
# which blanks content rather than credentials.
def scrub(answer: str) -> str:
    for name in ("OPENROUTER_API_KEY", "GH_TOKEN"):
        secret = os.environ.get(name)
        if secret:
            answer = answer.replace(secret, f"[redacted:{name}]")
    return answer


def main() -> int:
    issue = os.environ.get("ISSUE")
    if not issue:
        print("no issue number given — nowhere to post", file=sys.stderr)
        return 1
    session_id = latest_session_id()
    if session_id is None:
        print("no opencode session found to read the answer from", file=sys.stderr)
        return 1
    answer = final_answer(session_id).strip()
    if not answer:
        print("the agent changed nothing and said nothing either — no comment to post")
        return 0
    run_url = os.environ.get("RUN_URL", "")
    body = f"{scrub(answer)}\n\n[The run that answered this]({run_url})\n"
    return 0 if gh("issue", "comment", issue, "--body-file", "-", stdin=body) is not None else 1


if __name__ == "__main__":
    raise SystemExit(main())
