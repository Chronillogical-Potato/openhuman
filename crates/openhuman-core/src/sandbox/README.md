# sandbox

OpenHuman routes command execution and retains its existing shell, flow,
environment allowlist, and RPC surfaces. TinySecurity owns sandbox planning,
approval, admission, and single-use plan consumption. TinyBox owns native jail
facts and process execution.

## Command execution

The host translates the command, working directory, environment overrides,
runtime mode, and caller scope into a TinySecurity `Plan2` request. The plan
reply is the authority for backend selection and the native jail contract.
OpenHuman maps that reply to the existing `None`, `Local`, or `Docker`
executor, applies the existing component, config, and
`OPENHUMAN_SANDBOX` switches (the sandbox cannot be disabled for SaaS), and
consumes the plan immediately before starting the process. A refused,
unavailable, or unconsumable plan fails closed.

The host retains the `SANDBOX_ENV_PASSTHROUGH` allowlist. Caller overrides are
forwarded through the existing TinyBox adapter. `sandbox.*` RPC method names,
status fields, mode routing, and `is_remote` behavior remain stable.

## Native jail and grants

Backend reporting reads TinyBox `JailStatus`; OpenHuman does not probe or name
host jail implementations. The `sandbox.status` response preserves its
existing fields and reports an unavailable provider as `unavailable`.

TinySecurity supplies protected-path and credential-root policy to the
TinyBox local-grant resolver. TinyBox returns admitted grants, which OpenHuman
maps into the sandbox policy without a second host-owned protected-path floor.
The Plan2 jail artifact, including `deny_read`, is passed directly to TinyBox
at launch.

## Layout

- `ops/exec.rs` composes Plan2 planning, policy mapping, consumption, and the
  existing executor routing.
- `ops/policy.rs` retains host switches and backend status adapters.
- `policy.rs` maps agent mode and runtime facts to routing preferences.
- `spawn_plan/` maps host command facts and caller scope to TinySecurity.
- `native_jail.rs`, `local_grants.rs`, and `docker.rs` adapt TinyBox calls.
- `schemas.rs` preserves the `sandbox.*` RPC surface.

The settings surface remains in `config/ops/sandbox.rs`. Module internals and
their policy decisions are tested in TinySecurity and TinyBox; OpenHuman tests
the mapping, routing, switches, and module seams.
