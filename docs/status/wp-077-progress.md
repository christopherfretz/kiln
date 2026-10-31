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
- [x] Bench baseline `oven/virtual-dispatch`: 19.0 ms, 6.2 MB, 375540 steps,
      2176 bytes. Recorded at 21:26 EDT with nothing else running in the
      container (`ps`) and host loadavg 6.5 on 24 CPUs.
- [x] Full `cargo xtask ci` green on f94481a: kiln 1166 passed / 12 xfail /
      0 xpass / 0 failed (was 1159); clang self-check 1152 passed.

## Bench A/B against the merge base (125af7c)
Every counter (steps, bytes, evaluations, peak_arena, frozen_bytes) is
identical on every entry that has a baseline. An apparent ~3.5 ms fixed
per-process slowdown turned out to be a measurement artifact. The base
binary was in /tmp and the WP-077 binary was on the slow workspace mount;
loading a 42 MB binary from the mount costs ~3.5 ms. With both binaries in
/tmp: empty.cpp 1.17 vs 1.31 ms, fib 1.34 vs 1.47 ms, and churn-mixed 4.97 vs
5.03 ms (medians). The remaining ~0.1 ms comes from the larger binary.
For tooling: timings from `cargo xtask bench` include this mount cost.

## Next step
Nothing is left inside WP-077's scope. Resume with review and merge. The
follow-ups are listed under "Left for later" in spec/oven.md (virtual bases,
covariant returns through member pointers, freezing typeid pointers in
WP-080).
