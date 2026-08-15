import re
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent

SNAPSHOT_DIR = "crates/popcircles/src/snapshots"
REPORT = "crates/popcircles/src/report.rs"

# The two cache formats, and the blocks whose fields are their shape. Both structs of a ledger are
# listed because `Probe` is what `Document.radii` holds, so a field of either moves the document.
STRUCT_TRIGGERS = (
    ("crates/popcircles/src/table/cache.rs", ("Header",)),
    ("crates/popcircles/src/smallest/cache.rs", ("Document", "Probe")),
)

_KEY_RE = re.compile(r'^\s*"([A-Za-z0-9_]+)"\s*:', re.MULTILINE)
_INSTA_HEADER_RE = re.compile(r"\A---\n.*?\n---\n", re.DOTALL)
_FIELD_RE = re.compile(r"^\s*(?:pub )?([a-z_][A-Za-z0-9_]*)\s*:", re.MULTILINE)


@dataclass(frozen=True)
class Finding:
    trigger: str
    detail: str

    def __str__(self) -> str:
        return f"{self.trigger}: {self.detail}"


def git(*args: str) -> str | None:
    result = subprocess.run(  # noqa: S603 — argv, not a shell
        ["git", *args],  # noqa: S607 — trusted, repo-local, no user input
        cwd=REPO_ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    return result.stdout if result.returncode == 0 else None


def head(path: str) -> str | None:
    return git("show", f"HEAD:{path}")


# The index and not the working tree: what a commit will contain is what this has to answer about.
def index(path: str) -> str | None:
    return git("show", f":{path}")


# Unfiltered, so a staged deletion is in the list: a snapshot removed is a payload type withdrawn,
# which is the loudest non-additive change there is.
def staged_snapshots() -> list[str]:
    listing = git("diff", "--cached", "--name-only", "--", SNAPSHOT_DIR)
    return sorted(listing.splitlines()) if listing else []


def snapshot_keys(text: str) -> frozenset[str]:
    return frozenset(m.group(1) for m in _KEY_RE.finditer(_INSTA_HEADER_RE.sub("", text)))


def struct_fields(text: str, name: str) -> frozenset[str] | None:
    block = re.search(rf"^(?:pub )?struct {name} \{{\n(.*?)^\}}", text, re.MULTILINE | re.DOTALL)
    if block is None:
        return None
    return frozenset(m.group(1) for m in _FIELD_RE.finditer(block.group(1)))


# By value rather than by "the staged diff names the constant": editing the comment above it names
# it too, and a tripwire a comment satisfies reports having held when it did not.
def constant_value(text: str, name: str) -> str | None:
    m = re.search(rf"^(?:pub )?const {name}\s*:\s*\w+\s*=\s*([^;]+);", text, re.MULTILINE)
    return m.group(1).strip() if m else None


def bumped(path: str, constant: str) -> bool:
    before, after = head(path), index(path)
    if before is None or after is None:
        return False
    return constant_value(before, constant) != constant_value(after, constant)


def check_snapshots() -> list[Finding]:
    findings: list[Finding] = []
    for path in staged_snapshots():
        before = head(path)
        if before is None:
            # A new snapshot is a new payload type or a new case of one, and additive either way.
            continue
        after = index(path)
        # A key HEAD published and the staged document does not is what fires this, rather than any
        # modification: `report.rs` rules the format additive, so a new field legitimately rewrites
        # a snapshot and owes no bump, while a renamed or removed field always drops a key.
        gone = snapshot_keys(before) - (snapshot_keys(after) if after is not None else frozenset())
        if not gone or bumped(REPORT, "SCHEMA_VERSION"):
            continue
        what = "is gone" if after is None else f"no longer publishes {', '.join(sorted(gone))}"
        findings.append(Finding(path, f"{what}, under an unchanged SCHEMA_VERSION"))
    return findings


def check_structs() -> list[Finding]:
    findings: list[Finding] = []
    for path, names in STRUCT_TRIGGERS:
        before, after = head(path), index(path)
        if before is None or after is None:
            continue
        for name in names:
            old, new = struct_fields(before, name), struct_fields(after, name)
            if old is None:
                continue
            if new is None:
                # Zero fires too, for `single-unsafe-allow`'s reason: a block renamed out from under
                # this check leaves the check watching nothing and saying so nowhere.
                detail = f"no struct {name} — point STRUCT_TRIGGERS at the name it has now"
                findings.append(Finding(path, detail))
                continue
            if old == new or bumped(path, "FORMAT_VERSION"):
                continue
            # An added field fires this where a snapshot's key would not: serde ignores a key it
            # does not know, so a build reading a header from a later format accepts the document
            # and then maps a payload whose layout it has no reason to doubt.
            moved = sorted(f"+{f}" for f in new - old) + sorted(f"-{f}" for f in old - new)
            detail = f"fields {', '.join(moved)}, under an unchanged FORMAT_VERSION"
            findings.append(Finding(f"{path} struct {name}", detail))
    return findings


def main() -> int:
    findings = check_snapshots() + check_structs()
    for finding in findings:
        print(finding)
    if findings:
        print("\n  fix: bump that constant, or supersede the record the format's shape came from.")
        print(
            "  a change no existing reader can misread is a deliberate SKIP=version-bumps commit."
        )
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
