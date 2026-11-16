// GNU attributes on namespaces and in mixed sequences on declarations:
// `abi_tag` before or after the name of a namespace and of an inline
// namespace (ignored on a non-inline one, -Wignored-attributes), and a
// leading sequence that mixes `__attribute__((...))` and `[[...]]` on a
// simple-declaration, and an attribute-declaration whose specifiers are all
// empty (`[[]] [[]];`, silent) (GCC 16.1, clang 22). Found running GCC's C++
// testsuite (WP-1466).
//@ expect: compile-ok
//@ std: c++17
//@ strict-diags
//@ diag: 13:26 warning "'abi_tag' attribute on non-inline namespace ignored"
//@ diag: 15:29 warning "'abi_tag' attribute on non-inline namespace ignored"
namespace __attribute__((abi_tag("a"))) N1 { int x; }
inline namespace __attribute__((abi_tag("b"))) N2 { int y; }
namespace N3 __attribute__((abi_tag("c"))) { int z; }
inline namespace N4 __attribute__((abi_tag("d"))) { int w; }
__extension__ inline namespace N5 __attribute__((abi_tag("e"))) { int v; }
__attribute__(()) int a1;
int a2 __attribute__(());
__attribute__(()) [[]] __attribute__((unused)) [[]] int a3;
[[]] __attribute__((unused)) [[maybe_unused]] int a5;
[[]] [[]];
__attribute__(()) [[]];
[[]] [[]] void g();
