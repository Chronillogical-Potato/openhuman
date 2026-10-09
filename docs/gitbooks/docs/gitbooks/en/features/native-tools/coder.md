---
description: A complete toolset for working on real codebases - read, write, edit, search, git, lint, test.
icon: code
---

# Coder

The coder family is what makes OpenHuman a viable coding partner instead of a chat window that _pretends_ to know the codebase.

## Tools in the family

| Tool             | What it does                                                      |
| ---------------- | ----------------------------------------------------------------- |
| `file_read`      | Read a file (with line numbers, like `cat -n`).                   |
| `file_write`     | Write a new file.                                                 |
| `edit`           | Targeted edits - match-and-replace with strict uniqueness checks. |
| `apply_patch`    | Apply a unified diff.                                             |
| `glob`           | Find files by glob pattern.                                       |
| `grep`           | Ripgrep-style search across the tree.                             |
| `list`           | Walk a directory tree.                                            |
| `read_diff`      | Diff between two files or revisions.                              |
| `git_operations` | Status, diff, log, blame, branch, commit.                         |
| `run_linter`     | Run the project's linter.                                         |
| `run_tests`      | Run the project's test command.                                   |
| `csv_export`     | Export query results as CSV.                                      |

## Why these are native, not shell-only

A shell tool plus `cat`/`sed`/`awk` could _technically_ do all of this. The native tools exist because:

- Edits go through a uniqueness check, so the agent can't accidentally clobber the wrong line.
- Reads come back with line numbers the agent can refer to in follow-ups.
- Git operations parse output into structured data, instead of leaving the agent to scrape porcelain.
- Lint and test runs are wired to the project's actual commands, not generic guesses.

## Workspace scoping

Filesystem tools act inside the agent's **working folder** (`action_dir`), not the workspace directory, which holds internal state and is never a tool target. With the autonomy policy on, `workspace_only` confines them to that folder and anything outside it needs an explicit trusted root. With the policy off (the default) that confinement is not enforced, and the thing that still holds either way is the hard floor: credential stores (`~/.ssh`, `~/.gnupg`, `~/.aws`) and system roots are unreachable, as are `..` traversal and null bytes in a path. See [Approval Gate](../approval-gate.md).

## See also

- [System & Utilities](system-and-utilities.md) - `shell`, `node_exec`, `npm_exec`, `python_exec` for the rest of the dev loop.
- [Agent Coordination](agent-coordination.md) - `todo_write`, `spawn_subagent` for larger refactors.
