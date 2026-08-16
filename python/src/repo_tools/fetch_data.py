import hashlib
import os
import shutil
import sys
import urllib.request
from pathlib import Path

from population_circles.dataset_registry import REPO_ROOT, Registry, load

CHUNK = 1 << 20
FIX = "mise run data:get"

# A fetch writes into the tree, so the scheme is checked rather than trusted to the registry being
# well-formed. https only: the checksum catches corrupted bytes but not a downgrade that hands an
# attacker the chance to serve them.
SCHEME = "https://"


class FetchError(Exception):
    pass


def digest(path: Path) -> str:
    sha = hashlib.sha256()
    with path.open("rb") as handle:
        while chunk := handle.read(CHUNK):
            sha.update(chunk)
    return sha.hexdigest()


def matches(path: Path, size: int, sha256: str) -> bool:
    # Size first, so a truncated or partial download is rejected in a stat rather than by hashing it
    # into a mismatch, and a present 428 MB file is only hashed when it could possibly match.
    return path.is_file() and path.stat().st_size == size and digest(path) == sha256


def download(url: str, part: Path) -> None:
    if not url.startswith(SCHEME):
        message = f"refusing to fetch over a scheme that is not {SCHEME}: {url}"
        raise FetchError(message)
    with urllib.request.urlopen(url) as response, part.open("wb") as handle:  # noqa: S310
        shutil.copyfileobj(response, handle, CHUNK)


def fetch(url: str, target: Path, size: int, sha256: str) -> None:
    # Written beside the target and renamed only once the hash matches, so nothing that fails
    # verification is ever visible at the path a reader uses. The finally clause is what keeps an
    # interruption from leaving one behind.
    # The pid is in the name because two concurrent runs opening one fixed path share an inode: the
    # first rename hands the target to the second writer, which then keeps writing into the placed
    # file, and nothing has verified what a reader ends up with.
    part = target.with_name(f"{target.name}.{os.getpid()}.part")
    target.parent.mkdir(parents=True, exist_ok=True)
    try:
        download(url, part)
        # Length before the digest, and that order is the whole value of the check: a truncated
        # response raises nothing, since urlopen closes the connection rather than reporting a short
        # read, so this names the failure from a stat instead of a full pass over 428 MB.
        written = part.stat().st_size
        if written != size:
            message = (
                f"{target.name} arrived as {written} bytes, and the registry records {size} — "
                f"nothing was placed; re-run {FIX}"
            )
            raise FetchError(message)
        found = digest(part)
        if found != sha256:
            message = (
                f"{target.name} hashed to {found}, and the registry records {sha256} — "
                f"nothing was placed; re-run {FIX}"
            )
            raise FetchError(message)
        part.replace(target)
    finally:
        part.unlink(missing_ok=True)


def acquire(registry: Registry, root: Path) -> list[str]:
    owed: list[str] = []
    # Sorted, so two runs over one registry print the same lines in the same order.
    for key, dataset in sorted(registry.datasets.items()):
        target = dataset.file(root)
        if matches(target, dataset.bytes, dataset.sha256):
            print(f"{key}: verified, {dataset.bytes} bytes")
        elif dataset.fetch_url is None:
            # Committed rather than fetched, so a missing one is a damaged checkout and not
            # something this command can repair.
            message = (
                f"{key} is committed to this repository and {dataset.path} does not match it. "
                f"Restore it with `git checkout -- {dataset.path}`, or obtain it from "
                f"{dataset.source_url}"
            )
            raise FetchError(message)
        else:
            print(f"{key}: fetching {dataset.bytes} bytes")
            fetch(dataset.fetch_url, target, dataset.bytes, dataset.sha256)
            print(f"{key}: verified, {dataset.bytes} bytes")
        if dataset.attribution.strip():
            owed.append(f"{key} — {dataset.licence}:\n{dataset.attribution.strip()}")
    return owed


def main() -> int:
    try:
        owed = acquire(load(), REPO_ROOT)
    # URLError subclasses OSError, so one clause covers a failed request and a failed write.
    except (FetchError, OSError) as error:
        print(f"{error}", file=sys.stderr)
        return 1
    if owed:
        # Printed on success because acquiring the bytes is when the obligation is acquired —
        # ADR 0009's third consequence.
        print("\nPublishing anything derived from this data requires the attribution below.")
        for text in owed:
            print(f"\n{text}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
