import os

PREFIX = "AGENT_SECRET_"


# The convention, not a list: a step's own `env:` binds a value under this prefix beside its real
# name exactly when that value must not reach whatever this process is about to make public. A new
# secret gets scrubbed by adding its twin binding in the workflow — nothing here names one, because
# GitHub Actions has no way to enumerate a job's secrets short of naming each one somewhere.
def scrub(text: str) -> str:
    for name, value in os.environ.items():
        if name.startswith(PREFIX) and value:
            text = text.replace(value, f"[redacted:{name.removeprefix(PREFIX)}]")
    return text
