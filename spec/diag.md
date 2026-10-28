# spec: kiln_diag

Diagnostics are data first, text second. Every diagnostic has a stable
`DiagId` (an enum in `kiln_diag::ids`, grouped by component: `lex_*`, `pp_*`,
`parse_*`, `sema_*`, `oven_*`, `cg_*`, `drv_*`), a severity, a primary span,
optional secondary spans, notes, and structured arguments rendered through a
message template.

## Interface
- `DiagEngine`: collects diagnostics; policy for `-w`, `-Werror`, `-Werror=X`,
  `-Wno-X`, `-Wno-error=X`, `-ferror-limit=N`, `-fcolor-diagnostics`.
- `Diag::new(id, span).arg(..).note(span, text).emit(&engine)`.
- `Severity`: `Ignored | Remark | Warning | Error | Fatal`. Warnings belong to
  named groups (`-Wunused`, ...) via a static table; group names match clang's
  where the warning matches clang's.
- Rendering: clang-compatible text format
  `file:line:col: severity: message [-Wgroup]` followed by the source line and
  a caret/underline; `-fdiagnostics-format=json` for the harness (one JSON
  object per diagnostic with id, severity, file, line, col, message, notes).

## Invariants
- Emitting after the error limit sets `fatal` and later emits are dropped.
- Deterministic ordering: diagnostics are rendered in emission order; sema may
  buffer and sort by location within a declaration.
- No diagnostic is rendered twice for the same (id, span) in one TU unless the
  component asks (`allow_duplicate`).

## Tests
- Rendering goldens for single-line, multi-line, tab-containing, and
  macro-expansion diagnostics (with "expanded from macro" notes).
- Policy matrix for `-W*` flag combinations.
