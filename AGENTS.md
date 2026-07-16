# Agent Instructions

This project uses [metaswarm](https://github.com/dsifry/metaswarm), a multi-agent orchestration framework. It provides 18 specialized agents, a 9-phase development workflow, and quality gates that enforce TDD, coverage thresholds, and spec-driven development.

## How to Work in This Project

### Starting work

```text
$start
```

This is the default entry point. It primes the agent with relevant knowledge, guides you through scoping, and picks the right level of process for the task.

### For complex features (multi-file, spec-driven)

Describe what you want built, include a Definition of Done, and ask for the full workflow:

```text
I want you to build [description]. [Tech stack, DoD items, file scope.]
Use the full metaswarm orchestration workflow.
```

This triggers the full pipeline: Research, Plan, Design Review Gate, Work Unit Decomposition, Orchestrated Execution (4-phase loop per unit), Final Review, PR.

### Available Skills

Codex discovers skills by their SKILL.md `name` field. Invoke with `$name` syntax.

| Invoke | Purpose |
|---|---|
| `$start` | Begin tracked work on a task |
| `$setup` | Interactive guided setup |
| `$brainstorming-extension` | Refine an idea with design review gate |
| `$design-review-gate` | Trigger design review gate (5 reviewers) |
| `$plan-review-gate` | Adversarial plan review (3 reviewers) |
| `$orchestrated-execution` | 4-phase execution loop per work unit |
| `$pr-shepherd` | Monitor a PR through to merge |
| `$handling-pr-comments` | Handle PR review comments |
| `$create-issue` | Create a well-structured GitHub Issue |
| `$external-tools` | External AI tool delegation |
| `$status` | Run diagnostic checks |
| `$visual-review` | Playwright screenshot capture |

## Testing

- **TDD is mandatory** -- Write tests first, watch them fail, then implement
- **Coverage is a RATCHET FLOOR, not 100%** -- `.coverage-thresholds.json` holds the real per-tool floors (measured 2026-07-15: tarpaulin lines ~94.6%, llvm branches ~83%), just below measured so the gate is green today and blocks regression. The old 100% was fiction (nothing read the file). Raise floors as coverage improves; never lower without a committed justification. The Rust line floor is enforced in CI.
- Test command: `cargo test --workspace --all-features`
- Coverage command: `cargo tarpaulin --workspace --all-features --fail-under <thresholds.tarpaulin.lines>` (floor read from `.coverage-thresholds.json`; full local composite is `scripts/web/coverage.sh`)

## Coverage

Coverage thresholds are defined in `.coverage-thresholds.json` -- this is the **source of truth** for coverage requirements.
If a GitHub Issue specifies different coverage requirements, update `.coverage-thresholds.json` to match before implementation begins. Do not silently use a different threshold.

## Quality Gates

- **Design Review Gate**: 5-reviewer design review after design is drafted (`$design-review-gate`)
- **Plan Review Gate**: Adversarial review after any implementation plan is drafted. 3 independent reviewers (Feasibility, Completeness, Scope & Alignment) -- ALL must PASS before presenting the plan
- **Coverage Gate**: Reads `.coverage-thresholds.json` and runs the enforcement command -- BLOCKING gate before PR creation

## Workflow Enforcement (MANDATORY)

These rules override any conflicting instructions. They ensure the full metaswarm pipeline is followed.

### After Brainstorming

When brainstorming completes and commits a design document:

1. **STOP** -- do NOT proceed directly to planning or implementation
2. **RUN the Design Review Gate** -- invoke `$design-review-gate`
3. **WAIT** for all 5 reviewers (PM, Architect, Designer, Security, CTO) to approve
4. **ONLY THEN** proceed to planning/implementation

### After Any Plan Is Created

When a plan is produced:

1. **STOP** -- do NOT present the plan to the user or begin implementation
2. **RUN the Plan Review Gate** -- invoke `$plan-review-gate`
3. **WAIT** for all 3 adversarial reviewers to PASS
4. **ONLY THEN** present the plan to the user for approval

### Coverage Source of Truth

`.coverage-thresholds.json` is the **single source of truth** for coverage requirements. No skill may skip it.

### Agent Discipline

- **NEVER** use `--no-verify` on git commits
- **NEVER** use `git push --force` without explicit user approval
- **ALWAYS** follow TDD -- write tests first, watch them fail, then implement
- **STAY** within declared file scope

## External Tools (Optional)

If external AI tools are configured (`.metaswarm/external-tools.yaml`), the orchestrator can delegate implementation and review tasks to Codex CLI and Gemini CLI for cost savings and cross-model adversarial review.

## Guides

Development patterns and standards are documented in `guides/`:
- `agent-coordination.md` -- Agent dispatch patterns
- `build-validation.md` -- Build and validation workflow
- `coding-standards.md` -- Code style and conventions
- `git-workflow.md` -- Branching, commits, and PR conventions
- `testing-patterns.md` -- TDD patterns and coverage enforcement

## Code Quality

- Rust 2021; `cargo check --workspace --all-features`
- `cargo fmt --all -- --check` and strict Clippy (`-D warnings`)
- All quality gates must pass before PR creation
