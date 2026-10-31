# WP-112 progress

## Done
- [dcl.enum]/5 pre-brace enumerator types (the `<string_view>` "operator is
  ambiguous" ×84 came from glibc `_ISwbit` enumerators typed as the incomplete
  enum inside the body); test `dcl.enum/pre-brace-type.cpp`.
- Member noexcept-specifiers parsed late (complete-class context), `this` in
  the replayed spec, "not available until end of class definition"; the
  "needed" points of [except.spec]/13 (`exc_spec_needed`, also unevaluated);
  spec resolved before a DeclRef / member pointer takes the type.
- Lazy instantiation of dependent member `noexcept(expr)` (EST_Uninstantiated)
  with the "in instantiation of exception specification" note.
- Constructor templates as candidates; same/derived-class construction via
  full overload resolution (`resolve_ctor`); "selecting the constructor is not
  supported yet" gone; ctor-template mem-initializers instantiated.
- Diagnostics: aka exc-spec rule, LosesQuals "no known conversion" wording,
  Oven "implicit use of 'this'" note.
- Tests: `except.spec/` (3), `ast/except.spec/` (1), `over.match.ctor/` (2),
  `over.match.oper/` (2). Spec: sema.md §4.7/§5.3, sema-class.md §8.3.

## Next
- Re-probe `<array>`, `<string_view>`; full exit gate.

## Probe (kiln -fsyntax-only, libc++ 19)
| header | main | now |
|---|---:|---:|
| `<array>` | 180 | 115 (pre-ctor-selection) |
| `<string_view>` | 332 | 184 (pre-ctor-selection) |
