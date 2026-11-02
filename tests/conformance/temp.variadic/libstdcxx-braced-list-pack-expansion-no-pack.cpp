// [temp.variadic]/5: the pattern of a pack expansion names at least one
// unexpanded pack; a braced-init-list pattern without one is an error,
// in and out of a template (companion of
// libstdcxx-braced-list-pack-expansion.cpp, WP-332).
// Verified against clang 19.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 11:30 error "pack expansion does not contain any unexpanded parameter packs"
//@ diag: 12:52 error "pack expansion does not contain any unexpanded parameter packs"
struct P { int a; int b; };
void f() { P arr[2] = {{1, 2}...}; }
template <class T> void g(T t) { P arr[2] = {{t, 2}...}; }
