---
name: forge-spec-consistency
description: Audit a Forge Spec project for code-to-spec consistency, measure provider-derived adherence across every durable specification, populate only evidence-backed external attestations, drive explicitly requested adherence work through a scoped checkpoint commit and provider verification, and explain semantic, source-boundary, test, lifecycle, coverage, and traceability gaps. Use when asked to assess spec adherence, verify or populate implementation adherence, make an implementation checkpoint attestable, perform a code/spec consistency pass, investigate stale or unverified specifications, or produce a prioritized adherence gap report for a project with a .specs or specs tree.
---

# Forge Spec Consistency

Measure the provider's exact adherence state, independently review what the code
and tests actually demonstrate, and record attestations only when both evidence
layers support them. Keep provider state and semantic confidence separate.

## Choose the mode

- Treat `audit`, `measure`, `assess`, `review`, and `find gaps` as read-only.
- Treat `populate`, `verify`, `record`, and `attest` as authorization to append
  qualified adherence attestations. Do not infer authorization to commit,
  amend, revoke, migrate, push Git notes, or edit code/specs.
- Treat `checkpoint`, `commit and verify`, `finish adherence`, `make it
  attestable`, and an explicit request to bring adherence to `current` as
  authorization to complete the reviewed code/spec/test scope, create its
  checkpoint commit, and run provider verification. This does not authorize
  pushing commits or Git notes, rewriting history, migration, revocation, or
  unrelated cleanup.
- When asked to fix gaps, complete the audit first, agree on the intended
  contract where code and specs conflict, then use the project's normal
  implementation workflow.

## Establish a trustworthy baseline

1. Work from the Git project root. Read repository instructions and preserve
   unrelated changes.
2. Locate `.specs/` or `specs/`, inspect `_config.toml`, and run:

   ```sh
   command -v spec
   spec --version
   command -v forge-intellect
   forge-intellect --version
   spec migrate plan --target agent
   git status --short --branch
   git rev-parse HEAD
   spec lint
   spec implementation provider status
   spec implementation status --json
   ```

3. Prove executable provenance before trusting status. A version string alone is
   insufficient: a stale installed binary can report the expected version while
   implementing an older protocol. When auditing a forge-spec or forge-intellect
   checkout, prefer its documented repository-local wrapper or locked build and
   compare the returned snapshot schema with the protocol declared by the exact
   source/baseline. Forge Spec v0.6 requires `forge-spec-intellect/v2`. Treat a
   client/provider/source protocol mismatch as `unknown` and report each path,
   version, and schema involved.
4. Stop mutating work if the tree needs migration. Report the migration plan;
   apply it only when the user authorizes tracked changes. Continue a read-only
   audit when the installed CLI can safely read the older baseline.
5. Treat provider startup, loopback, lock, timeout, protocol, or synchronization
   failures as `unknown`, never as `unverified` or `current`. Diagnose the
   environment before blaming the code.
6. Explain that read-only mode means no tracked bytes, Git history, attestations,
   or remotes change. Provider queries may still create ephemeral lock, control,
   socket, and log metadata beside the Git administrative directory.
7. Capture raw JSON in a temporary directory outside the repository when the
   result will be reviewed or compared. This is a diagnostic artifact; report
   its path and remove it at handoff unless the user asks to retain it. Summarize
   the snapshot deterministically with:

   ```sh
   python3 <skill-dir>/scripts/summarize_adherence.py \
     --require-schema forge-spec-intellect/v2 <snapshot.json>
   ```

   Pass `--include-current` for the complete provider inventory. Use this helper
   only for counts and provider-state triage; it does not perform semantic review.

## Measure every durable specification

Do not sample when the user asks for all adherence. Exclude TASK work items from
adherence totals, then inspect them separately for workflow divergence.

For each durable specification:

1. Record its lifecycle, provider state, completeness, checkpoint, reasons, and
   evidence from the same snapshot.
2. Interpret provider states precisely:
   - `current`: complete attestation evidence still matches the declared intent
     and source boundary.
   - `unverified`: no active attestation exists.
   - `stale`: normative intent, declared boundary, or referenced source changed.
   - `partial`: only part of the required evidence resolved.
   - `violated`: explicit evidence records a deviation.
   - `unknown`: no safe evidence boundary could be established.
   - `unresolved`: a spec, source, checkpoint, or history object cannot resolve.
   - `not-applicable`: the entity has no direct code-adherence predicate.
3. Keep lifecycle independent. Draft, deprecated, and superseded specifications
   are not ordinary population targets even if code exists.
4. Inspect the durable hierarchy and the separate work-item view:

   ```sh
   spec inspect tree
   spec inspect tree --include-tasks
   spec inspect graph work
   ```

5. Surface accepted applicable specifications that are not `current`, but do
   not call every such state an implementation bug. State whether the gap is
   semantic, evidentiary, historical, environmental, or merely unrecorded.

## Review semantic consistency

Never equate provider `current` with proof that behavior is correct. The v0.6
provider establishes a conservative checkpoint and source-boundary proof; the
agent must still inspect behavior.

For every accepted, code-applicable durable specification:

1. Render the full context and inspect its relations and coverage:

   ```sh
   spec render <id> --target agent --include-source --include-docs
   spec inspect relations <id>
   spec inspect coverage <id>
   spec impact <id> --target agent
   ```

2. Enumerate every normative typed block and clause. Map each clause to exact
   source behavior and relevant automated tests. Prefer symbolic source refs;
   verify line refs still select the intended code.
3. Classify semantic evidence per clause:
   - `demonstrated`: implementation and focused test or directly inspectable
     invariant support the clause.
   - `plausible`: implementation appears consistent, but focused validation is
     absent or weak.
   - `gap`: code or tests omit or contradict the clause.
   - `unassessable`: the source, selector, runtime, or necessary evidence is
     unavailable.
4. Run the smallest relevant tests first, then broader project gates in
   proportion to risk. Distinguish passing tests from missing tests.
5. Inspect these additional consistency signals:
   - missing, overly broad, generated, vendored, or incomplete source boundaries;
   - accepted requirements with no implementation or test evidence;
   - stale/unresolved symbolic selectors and unstable line references;
   - `(violates)` history or implementation changes after the checkpoint;
   - `done` TASKs whose durable targets remain non-current or semantically gapped;
   - uncovered clause warnings, missing refinements, and lifecycle mismatches;
   - documentation cited as if it were implementation proof.

Use source refs, tests, and Git history as evidence. Do not invent relationships
or infer adherence from names, comments, TASK completion, or documentation alone.

## Drive an attestable checkpoint to completion

Use this workflow only when the user explicitly requests a checkpoint or asks
to finish the adherence work through verification.

1. Complete the baseline, full-scope measurement, and semantic review before
   editing. Freeze the exact durable IDs, clauses, source boundaries, tests, and
   tracked paths in scope. Keep unrelated changes out of the checkpoint.
2. Close every supported gap in that scope across specs, code, tests, and
   documentation. If the intended contract is genuinely ambiguous, pause for
   that decision; do not make the provider choose product intent.
3. Run focused validations followed by the repository's required broader gates.
   Re-run spec lint with the exact client that will perform verification. Do not
   commit with known semantic contradictions, unresolved source selectors,
   failing required tests, or a client/provider protocol mismatch.
4. Inspect the full diff and stage only the reviewed paths. Inspect the staged
   diff again, then create a checkpoint commit whose final trailer block contains
   exactly one `Spec-Ref: <id> (implements)` trailer for every applicable durable
   ID being attested. Exclude TASK and `not-applicable` entities. Do not use an
   empty checkpoint when the reviewed implementation changes can be committed.
5. Confirm the index and tracked worktree are clean at the new `HEAD`. If other
   user changes remain, prove they are outside every selected source boundary;
   otherwise the candidate is not safe to attest.
6. Run explicit verification for the reviewed IDs, or `--all` only after proving
   that every durable `(implements)` trailer on the exact commit was reviewed.
   Rerun provider status and require each selected ID to be `current` with
   complete evidence bound to that commit and source digest.
7. Confirm provider verification did not change tracked bytes or the index.
   Preserve the raw result until the final report, then clean temporary artifacts.
   Leave the commit and adherence notes local unless publication was requested.

## Populate qualified attestations

Populate only after completing the semantic review for the selected scope.

1. Select durable, accepted, code-applicable specifications with no known
   contradiction, an adequate declared source boundary, and proportionate test
   evidence. Skip `current` and `not-applicable` entries.
2. Require an exact clean candidate at `HEAD`. If the worktree is dirty, continue
   reporting but record nothing. Never stash, discard, or bundle unrelated work.
3. Inspect the candidate commit message. A fresh attestation requires an exact
   `Spec-Ref: <id> (implements)` trailer for every selected ID.
4. Do not fabricate trailers merely to make the report green. If review supports
   a dedicated checkpoint but `HEAD` lacks trailers, follow the checkpoint
   workflow directly when the user explicitly requested it. Otherwise present
   the reviewed ID list and request explicit commit authorization. A dedicated
   empty checkpoint commit is acceptable only after a complete review and
   explicit commit authorization.
5. Run explicit verification for the reviewed IDs, or use `--all` only when every
   durable `(implements)` trailer on the exact candidate was reviewed:

   ```sh
   spec implementation verify <id>...
   spec implementation verify --all
   ```

   `--all` means all durable IDs named by `(implements)` trailers on the exact
   candidate commit; it does not mean every specification in the project.
6. Rerun `spec implementation status --json`, summarize it, and confirm the
   selected IDs are `current` with complete evidence and attestation IDs. Confirm
   `HEAD`, the index, and tracked worktree bytes did not change.
7. Do not revoke existing attestations without an evidence-backed reason and
   explicit authorization. Do not run `migrate-attestations` without migration
   authorization. Do not push `refs/notes/forge-spec/adherence` unless the user
   explicitly requests publication.

## Report decision-first insights

Lead with the outcome and keep measured state distinct from reviewed semantics:

1. Give snapshot identity: spec executable path/version, provider path/version,
   protocol schema, full HEAD, clean or dirty worktree, snapshot completeness,
   baseline, and lint result.
2. Give counts for all durable specifications by provider state. State how many
   TASKs were excluded.
3. When a checkpoint was requested, give its commit hash, subject, reviewed path
   scope, exact `(implements)` ID set, and validation outcome. List attestations
   appended in this run with IDs, checkpoint, and attestation IDs. If none were
   safe, say so plainly.
4. Present a prioritized gap register. For each gap include the specification and
   clause, provider state, semantic classification, concrete code/test/history
   evidence, impact, and smallest next action.
5. Prioritize explicit MUST-level contradictions and `violated` states first;
   then stale/partial/unresolved evidence, accepted unimplemented clauses,
   missing focused tests, unverified checkpoints, and low-risk hierarchy or
   documentation hygiene.
6. Close with limitations: unavailable provider/runtime checks, dirty-state
   restrictions, untested behavior, areas reviewed statically only, and whether
   the commit or adherence notes remain unpublished.

Use language such as `provider-current, semantically demonstrated`,
`provider-current, semantic gap`, or `provider-unknown, review incomplete`.
Never collapse those independent conclusions into one green percentage.
