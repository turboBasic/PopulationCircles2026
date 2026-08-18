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
# with no second model call. `--sanitize` because the answer is about to become a public comment and
# the session held both the provider key and the installation token in its environment.
def final_answer(session_id: str) -> str:
    exported = opencode("export", session_id, "--sanitize")
    if not exported:
        return ""
    data = json.loads(exported)
    assistant = [m for m in data["messages"] if m["info"]["role"] == "assistant"]
    if not assistant:
        return ""
    parts = assistant[-1]["parts"]
    return "".join(part["text"] for part in parts if part["type"] == "text")


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
    body = f"{answer}\n\n[The run that answered this]({run_url})\n"
    return 0 if gh("issue", "comment", issue, "--body-file", "-", stdin=body) is not None else 1


if __name__ == "__main__":
    raise SystemExit(main())
