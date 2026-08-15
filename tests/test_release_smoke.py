import re
from pathlib import Path

import release_smoke as rs

WORKFLOW = Path(__file__).resolve().parent.parent / ".github/workflows/release.yml"

# gate and publish: the two jobs a tag owns and a dispatch must not run.
PUSH_ONLY_JOBS = 2


def workflow() -> str:
    return WORKFLOW.read_text(encoding="utf-8")


def subjects(found: list[str]) -> list[str]:
    return [finding.split(":")[0] for finding in found]


def test_the_expected_artifacts_are_the_workflow_matrix() -> None:
    triples = re.findall(r"^\s*triple:\s*(\S+)$", workflow(), re.MULTILINE)
    assert set(rs.ARTIFACTS) == {f"popcircles-{triple}" for triple in triples}


def test_the_publish_job_is_named_what_the_check_watches() -> None:
    assert re.search(rf"^\s*name:\s*{rs.PUBLISH_JOB}$", workflow(), re.MULTILINE)


# The three lines a dispatch rests on. A workflow that lost any of them still runs on a tag, so
# nothing else in this repository would report it.
def test_the_workflow_dispatches_and_guards_the_two_jobs_a_tag_owns() -> None:
    text = workflow()
    assert re.search(r"^\s*workflow_dispatch:$", text, re.MULTILINE)
    guards = re.findall(r"^\s*if: github\.event_name == 'push'$", text, re.MULTILINE)
    assert len(guards) == PUSH_ONLY_JOBS
    assert re.search(r"needs\.gate\.result != 'failure'", text)


def test_a_skipped_job_is_not_a_finding() -> None:
    skipped = [
        rs.Job("skipped", "Gate", "https://example.invalid/1"),
        rs.Job("success", "Build aarch64-apple-darwin", "https://example.invalid/2"),
        rs.Job("skipped", rs.PUBLISH_JOB, "https://example.invalid/3"),
    ]
    assert rs.findings(skipped, frozenset(rs.ARTIFACTS)) == []


def test_a_failed_leg_and_a_missing_artifact_are_each_named() -> None:
    failed = [rs.Job("failure", "Build aarch64-apple-darwin", "https://example.invalid/2")]
    found = rs.findings(failed, frozenset({rs.ARTIFACTS[1]}))
    assert subjects(found) == ["Build aarch64-apple-darwin", rs.ARTIFACTS[0]]
    assert "failure" in found[0]


def test_a_publish_that_ran_is_a_finding_of_its_own() -> None:
    published = [rs.Job("success", rs.PUBLISH_JOB, "https://example.invalid/3")]
    [finding] = rs.findings(published, frozenset(rs.ARTIFACTS))
    assert finding.startswith(rs.PUBLISH_JOB)
    assert "may have published" in finding
