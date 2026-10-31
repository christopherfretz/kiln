# WP-112 progress

## Done
- [dcl.enum]/5 pre-brace enumerator types (the `<string_view>` "operator is
  ambiguous" ×84 came from glibc `_ISwbit` enumerators typed as the incomplete
  enum inside the body); test `dcl.enum/pre-brace-type.cpp`.
- Member noexcept-specifiers parsed late: `ExcSpec::Deferred(f)` + `unparsed_exc`
  at declaration, `act_on_exception_spec` sets the written spec; `this` in the
  replayed spec; "exception specification is not available until end of class
  definition" on early use.

## Next
- Lazy instantiation of dependent member exception specs (clang EST_Uninstantiated).
- "selecting the constructor is not supported yet" (<array> 12).
- Tests, probe, spec.

## Probe (kiln -fsyntax-only, libc++ 19)
| header | main | now |
|---|---:|---:|
| `<array>` | 180 | 115 |
| `<string_view>` | 332 | 184 |
