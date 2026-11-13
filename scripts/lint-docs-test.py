#!/usr/bin/env python3
"""Tests of lint-docs.py's ADR rules: which changes to an accepted ADR the `change` check
lets through (ADR 0001, ADR 0018), and which Corrected lines the `adrs` check refuses.
Standard library only, no git and no network; `just test-scripts` runs it.

    python3 scripts/lint-docs-test.py
"""
import importlib.util
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("lint_docs", HERE / "lint-docs.py")
ld = importlib.util.module_from_spec(spec)
spec.loader.exec_module(ld)

ADR = """# 0042. Read the key from a file, never argv

- **Status:** Accepted
- **Date:** 2026-11-01
- **Invariants affected:** 2, 5
- **Supersedes:** none

## Context

The key was read on 2026-10-30 by the owner.

## Decision

Muster reads the key from a file.

```text
## not a heading: inside a fenced block
```

## Consequences

Nothing on argv.
"""

CORRECTION = "- **Corrected:** 2026-11-13: the Context said 2026-10-30; it was 2026-10-31.\n"


def corrected(text: str, line: str = CORRECTION, fix: bool = True) -> str:
    """`text` with `line` appended to its header and, when `fix`, the Context's date
    changed, as a correction of fact does."""
    out = text.replace("\n\n## Context", "\n" + line.rstrip("\n") + "\n\n## Context", 1)
    return out.replace("read on 2026-10-30", "read on 2026-10-31") if fix else out


class AdrChange(unittest.TestCase):
    def allowed(self, before: str, after: str) -> None:
        self.assertIsNone(ld.adr_change_problem(before, after))

    def refused(self, before: str, after: str, why: str) -> None:
        problem = ld.adr_change_problem(before, after)
        self.assertIsNotNone(problem)
        self.assertIn(why, problem)

    def test_an_unchanged_adr_passes(self):
        self.allowed(ADR, ADR)

    def test_the_status_line_may_change(self):
        self.allowed(ADR, ADR.replace("Accepted", "Superseded by 0043", 1))
        self.allowed(ADR, ADR.replace("Accepted", "Accepted; superseded in part by 0043", 1))

    def test_an_edit_without_a_corrected_entry_is_refused(self):
        self.refused(ADR, ADR.replace("read on 2026-10-30", "read on 2026-10-31"), "without a new Corrected entry")

    def test_a_correction_of_fact_passes(self):
        self.allowed(ADR, corrected(ADR))

    def test_a_correction_may_span_several_lines(self):
        self.allowed(ADR, corrected(ADR, "- **Corrected:** 2026-11-13: the Context said\n  2026-10-30; it was 2026-10-31.\n"))

    def test_a_correction_never_changes_the_decision(self):
        after = corrected(ADR).replace("from a file.", "from argv.")
        self.refused(ADR, after, "Decision section changed")

    def test_a_correction_never_changes_the_title(self):
        after = corrected(ADR).replace("never argv", "or argv")
        self.refused(ADR, after, "title changed")

    def test_a_correction_never_changes_another_header_field(self):
        after = corrected(ADR).replace("- **Supersedes:** none", "- **Supersedes:** 0041")
        self.refused(ADR, after, "header field other than Status and Corrected")

    def test_a_corrected_line_outside_the_header_does_not_count(self):
        after = ADR.replace("Nothing on argv.", "Nothing on argv.\n" + CORRECTION).replace("read on 2026-10-30", "read on 2026-10-31")
        self.refused(ADR, after, "without a new Corrected entry")

    def test_a_second_correction_keeps_the_first_whole(self):
        first = corrected(ADR, "- **Corrected:** 2026-11-13: the Context said\n  2026-10-30; it was 2026-10-31.\n")
        second = first.replace("Nothing on argv.", "Nothing on argv, ever.")
        second = second.replace("\n\n## Context", "\n- **Corrected:** 2026-11-14: a second fact.\n\n## Context", 1)
        self.allowed(first, second)
        reworded = second.replace("  2026-10-30; it was 2026-10-31.", "  a date.")
        self.assertNotEqual(reworded, second)
        self.refused(first, reworded, "earlier Corrected entry changed or moved")

    def test_the_decision_section_ends_at_a_heading_outside_a_fence(self):
        lines = ld.decision_section(ADR)
        self.assertIn("## not a heading: inside a fenced block", lines)
        self.assertNotIn("Nothing on argv.", lines)


class CorrectedLines(unittest.TestCase):
    def test_a_well_formed_line_in_the_header_passes(self):
        self.assertEqual(ld.corrected_problems(corrected(ADR)), [])

    def test_a_misspelt_line_is_named(self):
        text = corrected(ADR, "- **Corrected**: 2026-11-13: the colon is outside the bold.\n")
        self.assertIn("not in the form", " ".join(ld.corrected_problems(text)))

    def test_a_date_that_is_not_a_date_is_named(self):
        text = corrected(ADR, "- **Corrected:** 2026-13-45: no such month.\n")
        self.assertIn("is not a date", " ".join(ld.corrected_problems(text)))

    def test_a_line_outside_the_header_is_named(self):
        text = ADR.replace("Nothing on argv.", "Nothing on argv.\n" + CORRECTION)
        self.assertIn("outside its header", " ".join(ld.corrected_problems(text)))

    def test_a_partly_superseded_adr_is_still_accepted(self):
        self.assertEqual(ld.adr_status(ADR.replace("Accepted", "Accepted; superseded in part by 0043, 0044", 1)), "Accepted")


if __name__ == "__main__":
    unittest.main()
