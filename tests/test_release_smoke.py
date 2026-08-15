import re
from pathlib import Path

import release_smoke as rs

WORKFLOWS = Path(__file__).resolve().parent.parent / ".github/workflows"


def workflow(name: str) -> str:
    return (WORKFLOWS / name).read_text(encoding="utf-8")


def subjects(found: list[str]) -> list[str]:
    return [finding.split(":")[0] for finding in found]


def test_the_expected_artifacts_are_the_build_matrix() -> None:
    triples = re.findall(r"^\s*triple:\s*(\S+)$", workflow("build-binaries.yml"), re.MULTILINE)
    assert set(rs.ARTIFACTS) == {f"popcircles-{triple}" for triple in triples}


def test_the_smoke_dispatches_the_workflow_this_script_watches() -> None:
    text = workflow(rs.WORKFLOW)
    assert re.search(r"^\s*workflow_dispatch:$", text, re.MULTILINE)
    assert "uses: ./.github/workflows/build-binaries.yml" in text


# ADR 0010 decision 2: a smoke publishes nothing because its workflow declares no job that could,
# and neither wrapper tells its jobs apart by the event. Both are properties of the files, so this
# is where they are checked — at run time there is nothing left to look at.
def test_the_smoke_workflow_can_neither_publish_nor_write() -> None:
    text = workflow(rs.WORKFLOW)
    assert "contents: write" not in text
    assert "gh release" not in text


def test_neither_wrapper_forks_on_the_event() -> None:
    for name in (rs.WORKFLOW, "release.yml"):
        assert "github.event_name" not in workflow(name)


def test_a_skipped_job_is_not_a_finding() -> None:
    skipped = [
        rs.Job("skipped", "Gate", "https://example.invalid/1"),
        rs.Job("success", "build / Build aarch64-apple-darwin", "https://example.invalid/2"),
    ]
    assert rs.findings(skipped, frozenset(rs.ARTIFACTS)) == []


def test_a_failed_leg_and_a_missing_artifact_are_each_named() -> None:
    failed = [rs.Job("failure", "build / Build aarch64-apple-darwin", "https://example.invalid/2")]
    found = rs.findings(failed, frozenset({rs.ARTIFACTS[1]}))
    assert subjects(found) == ["build / Build aarch64-apple-darwin", rs.ARTIFACTS[0]]
    assert "failure" in found[0]
