import json
from pathlib import Path
from typing import Any

import pytest

from population_circles.circle_document import CIRCLE_KINDS, Envelope, circle_of

# The only check a committed document faces (ADR 0011), and through the reader's own boundary rather
# than a second parser — so a schema bump fails here rather than in a gallery build.
CORPUS = sorted((Path(__file__).resolve().parents[2] / "results").glob("*.json"))


def test_the_corpus_holds_documents() -> None:
    assert CORPUS


@pytest.mark.parametrize("document", CORPUS, ids=[path.stem for path in CORPUS])
def test_a_committed_document_parses_and_names_its_dataset(document: Path) -> None:
    payload: Any = json.loads(document.read_text(encoding="utf-8"))
    envelope = Envelope.model_validate(payload)
    assert envelope.provenance is not None
    assert envelope.provenance.dataset is not None
    if envelope.document in CIRCLE_KINDS:
        circle_of(payload)
