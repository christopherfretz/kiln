// [namespace.def] with GNU `abi_tag` (WP-384): clang accepts the attribute
// only on a named inline namespace; on a non-inline or anonymous namespace
// it is ignored with a warning (and tags nothing, spec/abi.md §8.15).
// Verified against clang 19.
//@ strict-diags
//@ diag: 9:26 warning "'abi_tag' attribute on non-inline namespace ignored"
//@ diag: 10:33 warning "'abi_tag' attribute on anonymous namespace ignored"
//@ diag: 11:13 warning "'abi_tag' attribute on non-inline namespace ignored"
namespace __attribute__((abi_tag("a"))) A { struct S {}; }
inline namespace __attribute__((abi_tag("b"))) { struct T {}; }
namespace [[gnu::abi_tag("c")]] B { struct U {}; }
inline namespace I __attribute__((abi_tag("d"))) { struct V {}; }
A::S f1() { return {}; }
B::U f2() { return {}; }
V f3() { return {}; }
