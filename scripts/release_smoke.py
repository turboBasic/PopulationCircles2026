import subprocess
import sys
import time
from dataclasses import dataclass
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent

WORKFLOW = "release-smoke.yml"

# One per leg of the build matrix `release-smoke.yml` calls. Pinned here rather than read back off
# whatever the run happened to upload, because what this command exists to prove is that the macOS
# leg produced something: a run whose matrix had lost that leg would otherwise come back green with
# one artifact. tests/test_release_smoke.py holds these against the workflow's own matrix.
ARTIFACTS = ("popcircles-aarch64-apple-darwin", "popcircles-x86_64-unknown-linux-gnu")

UNEVENTFUL = frozenset({"success", "skipped"})

APPEAR_TIMEOUT_S = 90
POLL_S = 3


@dataclass(frozen=True)
class Job:
    conclusion: str
    name: str
    url: str


def git(*args: str) -> str | None:
    result = subprocess.run(  # noqa: S603 — argv, not a shell
        ["git", *args],  # noqa: S607 — trusted, repo-local
        cwd=REPO_ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    return result.stdout.strip() if result.returncode == 0 else None


def gh(*args: str) -> str | None:
    result = subprocess.run(  # noqa: S603 — argv, not a shell
        ["gh", *args],  # noqa: S607 — trusted; the only caller-supplied value is a ref
        cwd=REPO_ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode != 0:
        print(result.stderr.strip(), file=sys.stderr)
        return None
    return result.stdout.strip()


# The dispatch builds the ref as origin has it, so a tree with unpushed work gets evidence about a
# commit that is not the one in hand — green, and about something else.
def unpushed(ref: str, *, on_this_branch: bool) -> str | None:
    listing = git("ls-remote", "origin", f"refs/heads/{ref}")
    if not listing:
        return f"origin has no branch {ref}, and a dispatch builds the ref on origin"
    remote = listing.split()[0]
    local = git("rev-parse", "HEAD")
    if on_this_branch and local is not None and local != remote:
        return f"origin/{ref} is at {remote[:12]} and HEAD here is {local[:12]}"
    return None


def latest_run(ref: str) -> tuple[int, str] | None:
    out = gh(
        "run",
        "list",
        "--workflow",
        WORKFLOW,
        "--event",
        "workflow_dispatch",
        "--branch",
        ref,
        "--limit",
        "1",
        "--json",
        "databaseId,url",
        "--jq",
        r'.[] | "\(.databaseId) \(.url)"',
    )
    if not out:
        return None
    number, url = out.split()
    return int(number), url


# By identifier and not by "the newest run": a dispatch answers before its run is queryable, and the
# newest run of a branch that has been smoked before is the previous one until it is not.
def wait_for_new_run(ref: str, baseline: int) -> tuple[int, str] | None:
    deadline = time.monotonic() + APPEAR_TIMEOUT_S
    while time.monotonic() < deadline:
        time.sleep(POLL_S)
        found = latest_run(ref)
        if found is not None and found[0] > baseline:
            return found
    print(f"no new run of {WORKFLOW} appeared within {APPEAR_TIMEOUT_S}s", file=sys.stderr)
    return None


def jobs(run_id: int) -> list[Job]:
    out = gh(
        "run",
        "view",
        str(run_id),
        "--json",
        "jobs",
        "--jq",
        r'.jobs[] | "\(.conclusion)\t\(.name)\t\(.url)"',
    )
    if out is None:
        return []
    return [Job(*line.split("\t", 2)) for line in out.splitlines() if line]


def artifacts(run_id: int) -> frozenset[str]:
    out = gh(
        "api",
        f"repos/{{owner}}/{{repo}}/actions/runs/{run_id}/artifacts",
        "--jq",
        ".artifacts[].name",
    )
    return frozenset(out.splitlines()) if out else frozenset()


def findings(run_jobs: list[Job], uploaded: frozenset[str]) -> list[str]:
    found = [
        f"{job.name}: {job.conclusion}\n  {job.url}"
        for job in run_jobs
        if job.conclusion not in UNEVENTFUL
    ]
    found += [f"{name}: no such artifact on the run" for name in ARTIFACTS if name not in uploaded]
    return found


def main() -> int:
    requested = sys.argv[1] if len(sys.argv) > 1 else None
    branch = git("rev-parse", "--abbrev-ref", "HEAD")
    ref = requested or branch
    if ref is None or ref == "HEAD":
        print("no branch to smoke: name one, or check one out", file=sys.stderr)
        return 1

    stale = unpushed(ref, on_this_branch=ref == branch)
    if stale is not None:
        print(
            f"{stale}\n  fix: push it — a dispatch builds origin's ref, not this tree",
            file=sys.stderr,
        )
        return 1

    before = latest_run(ref)
    if gh("workflow", "run", WORKFLOW, "--ref", ref) is None:
        print(
            f"  a workflow_dispatch trigger counts only where the default branch declares it,\n"
            f"  so {WORKFLOW} has to carry one on main before {ref} can be smoked.",
            file=sys.stderr,
        )
        return 1

    started = wait_for_new_run(ref, before[0] if before else 0)
    if started is None:
        return 1
    run_id, url = started
    print(f"smoking {ref} — {url}")

    # Progress only. The verdict below is read back from the run, so a watch that cannot attach — a
    # run that finished first, a terminal that is not one — costs the check nothing.
    subprocess.run(  # noqa: S603 — argv, not a shell
        ["gh", "run", "watch", str(run_id), "--exit-status"],  # noqa: S607 — trusted, repo-local
        cwd=REPO_ROOT,
        check=False,
    )

    found = findings(jobs(run_id), artifacts(run_id))
    if found:
        print(f"\nthe smoke of {ref} failed — {url}", file=sys.stderr)
        for finding in found:
            print(f"  {finding}", file=sys.stderr)
        print("\n  nothing was tagged and nothing was published; the run page is all this left.")
        return 1

    print(f"\nboth legs compiled and uploaded — {url}")
    for name in ARTIFACTS:
        print(f"  {name}")
    print("  no tag, no Release: this proves the build, not the publish job a tag runs.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
