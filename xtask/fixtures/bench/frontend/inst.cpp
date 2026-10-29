// Instantiation-counter entry for `cargo xtask bench` (the `inst.*` metrics,
// spec/sema-templates.md §11.4), compiled by the fake kiln
// (`xtask/fixtures/fake-kiln/kiln`), which reports `inst.cpp.inst.jsonl` on
// stderr and the empty constexpr report `inst.cpp.oven.jsonl`.
//@ std: c++20
template <int N> struct R { static constexpr int v = R<N - 1>::v + 1; };
template <> struct R<0> { static constexpr int v = 0; };
int x = R<3>::v;
