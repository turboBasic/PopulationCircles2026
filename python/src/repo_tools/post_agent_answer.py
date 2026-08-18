import os
import subprocess
import sys
from pathlib import Path

from repo_tools.capture_agent_answer import ANSWER_FILE
from repo_tools.redact import scrub


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


def main() -> int:
    issue = os.environ.get("ISSUE")
    if not issue:
        print("no issue number given — nowhere to post", file=sys.stderr)
        return 1
    answer_file = Path(os.environ["RUNNER_TEMP"], ANSWER_FILE)
    if not answer_file.is_file():
        print("no captured answer — the agent changed nothing and said nothing either")
        return 0
    # Scrubbed once already by the step that captured it, against what that step held. Scrubbed
    # again here against what this step holds — the app token, present because posting needs it
    # anyway, never fetched for this alone.
    answer = scrub(answer_file.read_text())
    run_url = os.environ.get("RUN_URL", "")
    body = f"{answer}\n\n[The run that answered this]({run_url})\n"
    return 0 if gh("issue", "comment", issue, "--body-file", "-", stdin=body) is not None else 1


if __name__ == "__main__":
    raise SystemExit(main())
