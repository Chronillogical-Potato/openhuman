# Security

The core security family owns host policy context, approvals, credential
handling, secret scrubbing, and RPC listener protection. TinySecurity owns the
shared command, path, and action-budget decisions used by the host.

## Host and module boundary

`SecurityPolicy` carries the autonomy setting, workspace and action roots,
trusted roots, and the current per-turn scope. `modules/security/gap.rs`
forwards that context to TinySecurity and fails closed when the module cannot
answer. OpenHuman does not maintain a duplicate shell command vocabulary or
path-protection floor.

- Shell requests go to TinySecurity command authorization using the actual
  command text, flow, and stage. Other tools provide their declared permission
  and external-effect properties.
- Path checks use TinySecurity's batched path checks and path/parent validators.
  The host keeps path-string well-formedness checks and OS-specific filesystem
  operations such as existence and symlink resolution.
- Action spending and budget snapshots come from TinySecurity. Sync-only host
  traits read a cached module answer populated by async admission.
- Approval prompts remain a host concern. TinySecurity's class/gate result is
  adapted to the existing approval and refusal contract.

The autonomy policy is disabled by default in product configuration. Always-on
credential and system path floors, traversal checks, and null-byte checks are
owned by TinySecurity and remain active independently of that setting.

See [`policy/README.md`](./policy/README.md) for policy adapters and the
[security architecture](../../../../docs/gitbooks/en/developing/architecture/security.md)
for the wider system boundary.
