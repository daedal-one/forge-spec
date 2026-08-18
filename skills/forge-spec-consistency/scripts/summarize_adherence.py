#!/usr/bin/env python3
"""Summarize a `spec implementation status --json` snapshot as Markdown."""

from __future__ import annotations

import argparse
import json
import sys
from collections import Counter
from pathlib import Path
from typing import Any, TextIO


STATE_ORDER = (
    "current",
    "unverified",
    "stale",
    "partial",
    "violated",
    "unknown",
    "unresolved",
    "not-applicable",
)
NON_GAP_STATES = {"current", "not-applicable"}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Summarize Forge Spec provider adherence JSON as Markdown."
    )
    parser.add_argument(
        "snapshot",
        nargs="?",
        default="-",
        help="Snapshot path, or - for standard input (default: -).",
    )
    parser.add_argument(
        "--include-current",
        action="store_true",
        help="Include current and not-applicable specifications in the table.",
    )
    parser.add_argument(
        "--require-schema",
        help="Fail unless the snapshot uses this exact protocol schema.",
    )
    return parser.parse_args()


def open_snapshot(path: str) -> TextIO:
    if path == "-":
        return sys.stdin
    return Path(path).open(encoding="utf-8")


def require_mapping(value: Any, label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise ValueError(f"{label} must be a JSON object")
    return value


def require_specifications(value: Any) -> list[dict[str, Any]]:
    if not isinstance(value, list):
        raise ValueError("specifications must be a JSON array")
    specifications = []
    for index, item in enumerate(value):
        specifications.append(require_mapping(item, f"specifications[{index}]"))
    return specifications


def markdown(value: Any) -> str:
    text = "" if value is None else str(value)
    return " ".join(text.split()).replace("|", "\\|")


def short_object_id(value: Any) -> str:
    text = markdown(value)
    return text if len(text) <= 12 else text[:12]


def ordered_states(counts: Counter[str]) -> list[str]:
    known = [state for state in STATE_ORDER if state in counts]
    unknown = sorted(set(counts) - set(STATE_ORDER))
    return known + unknown


def summarize(
    snapshot: dict[str, Any], include_current: bool, required_schema: str | None = None
) -> str:
    schema = snapshot.get("schema", "unknown")
    if not isinstance(schema, str) or not schema.startswith("forge-spec-intellect/"):
        raise ValueError("snapshot schema is not a Forge Spec intellect schema")
    if required_schema is not None and schema != required_schema:
        raise ValueError(
            f"snapshot schema {schema!r} does not match required schema "
            f"{required_schema!r}"
        )

    workspace = require_mapping(snapshot.get("workspace", {}), "workspace")
    specifications = require_specifications(snapshot.get("specifications"))
    specifications.sort(key=lambda item: str(item.get("id", "")))

    counts: Counter[str] = Counter(
        str(item.get("state", "missing")) for item in specifications
    )
    reasons: Counter[str] = Counter(
        str(reason)
        for item in specifications
        for reason in item.get("reasons", [])
        if str(reason).strip()
    )

    head = markdown(workspace.get("head", "unknown"))
    worktree = markdown(workspace.get("worktree", "unknown"))
    worktree_label = (
        "clean" if worktree == "clean" else f"dirty ({short_object_id(worktree)})"
    )
    complete = snapshot.get("complete") is True

    lines = [
        "# Forge Spec adherence snapshot",
        "",
        f"- Schema: `{markdown(schema)}`",
        f"- Provider: `{markdown(snapshot.get('provider', 'unknown'))}` "
        f"`{markdown(snapshot.get('provider_version', 'unknown'))}`",
        f"- Workspace: `{markdown(workspace.get('root', 'unknown'))}`",
        f"- HEAD: `{head}`",
        f"- Worktree: {worktree_label}",
        f"- Snapshot complete: {'yes' if complete else 'no'}",
        f"- Durable specifications: {len(specifications)}",
        "",
        "## State counts",
        "",
        "| State | Count |",
        "|---|---:|",
    ]
    for state in ordered_states(counts):
        lines.append(f"| `{markdown(state)}` | {counts[state]} |")

    if reasons:
        lines.extend(
            [
                "",
                "## Provider signals",
                "",
                "| Occurrences | Reason |",
                "|---:|---|",
            ]
        )
        for reason, count in sorted(
            reasons.items(), key=lambda item: (-item[1], item[0])
        ):
            lines.append(f"| {count} | {markdown(reason)} |")

    selected = [
        item
        for item in specifications
        if include_current or str(item.get("state", "missing")) not in NON_GAP_STATES
    ]
    inventory_title = (
        "## Specification inventory" if include_current else "## Provider-state gaps"
    )
    lines.extend(["", inventory_title, ""])
    if not selected:
        lines.append("No provider-state gaps are present in this snapshot.")
        return "\n".join(lines) + "\n"

    lines.extend(
        [
            "| Specification | State | Complete | Checkpoint | Evidence | First reason |",
            "|---|---|:---:|---|---:|---|",
        ]
    )
    for item in selected:
        item_reasons = item.get("reasons", [])
        first_reason = (
            item_reasons[0]
            if isinstance(item_reasons, list) and item_reasons
            else ""
        )
        if first_reason and reasons[str(first_reason)] > 1:
            first_reason = "repeated provider signal; see above"
        evidence = item.get("evidence", [])
        evidence_count = len(evidence) if isinstance(evidence, list) else 0
        lines.append(
            "| "
            f"`{markdown(item.get('id', 'missing'))}` | "
            f"`{markdown(item.get('state', 'missing'))}` | "
            f"{'yes' if item.get('complete') is True else 'no'} | "
            f"`{short_object_id(item.get('checkpoint')) or '-'}` | "
            f"{evidence_count} | {markdown(first_reason)} |"
        )

    return "\n".join(lines) + "\n"


def main() -> int:
    args = parse_args()
    try:
        with open_snapshot(args.snapshot) as handle:
            snapshot = require_mapping(json.load(handle), "snapshot")
        sys.stdout.write(
            summarize(snapshot, args.include_current, required_schema=args.require_schema)
        )
    except (OSError, json.JSONDecodeError, ValueError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
