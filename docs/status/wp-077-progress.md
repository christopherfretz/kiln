# WP-077 progress: virtual dispatch, virtual bases, dynamic_cast, typeid

Branch `wp/077-oven-virtual`. Started 2026-10-30 20:45 EDT. The design as
landed is in spec/oven.md "Dynamic types — Landed (WP-077)".

## Status
- [x] Dynamic-type words: SetVptr in ctors and dtors, thaw reconstruction
- [x] VCall through final_overrider, with a per-(class, offset, method) cache
- [x] Notes for pure virtual calls, CWG1517, lifetime, and unknown dynamic type
- [x] Virtual member-pointer calls
- [x] Covariant returns (CovAdj)
- [x] delete through a virtual dtor, including implicit dtors defined on demand
- [x] DeleteNonVirtual note
- [x] dynamic_cast: pointer, reference, void*, and all four clang failure kinds
- [x] typeid: static, dynamic, and during construction
- [x] Virtual bases, minimal: still trap; objects with virtual bases cannot be
      constructed, so dispatch never meets one
- [x] Tests: `tests/conformance/expr.const/virtual-{dispatch,errors,dynamic-cast,dynamic-cast-errors,typeid,typeid-errors,delete}.cpp`
- [x] Bench entry `bench/oven/virtual-dispatch.cpp`
- [x] Spec section
- [ ] Bench baseline: record only on a quiet host
- [ ] Full `cargo xtask ci`

## Next step
Run the full `cargo xtask ci` (in the background). Then record the
`virtual-dispatch` baseline with
`cargo xtask bench --filter oven/virtual-dispatch --record`, but only when
`ps` shows a quiet host.
