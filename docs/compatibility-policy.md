# Compatibility Policy

Default policy: preserve and warn.

AgentSync should never silently drop behavior. When one agent has a feature that
another agent cannot represent, AgentSync preserves native data where possible
and reports the gap in `scan`, `status`, `diff`, and `doctor`.

## Support Levels

- `read`: discover and display the resource.
- `write`: generate the native target format.
- `sync`: track drift and reconcile changes.
- `partial`: render supported fields, but warn about unmapped fields.
- `unsupported`: do not render this resource or field.

## Blocking Rules

Unsupported cosmetic fields can warn without blocking writes. Unsupported
behavioral or security-sensitive fields block writes unless the user explicitly
allows a lossy conversion. Examples include permissions, sandbox policies,
executable hooks, lifecycle behavior, and plugin code.

Hook resources use the narrower policy in `docs/hook-equivalence-policy.md`.
AgentSync may render directly equivalent hook entries while reporting unmapped
entries, but it must not silently drop unsupported hook events, matchers, or
handler fields.

## User-Facing Labels

Compatibility output should use clear labels:

- `portable`: all meaningful fields map.
- `partial`: some fields are preserved but not rendered.
- `blocked`: the target cannot represent required behavior.
