// [dcl.attr.grammar]/5: clang's mutually exclusive attribute pairs hot/cold
// and no_destroy/always_destroy are errors when both appertain to the same
// entity, on one declaration or across redeclarations; the later attribute
// is the one diagnosed and dropped (WP-890, WP-891; spec/sema.md).
// Verified against clang 19 and 22.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 31:21 error "'cold' and 'hot' attributes are not compatible"
//@ diag: 31:16 note "conflicting attribute is here"
//@ diag: 32:22 error "hot' and 'cold' attributes are not compatible"
//@ diag: 32:16 note "conflicting attribute is here"
//@ diag: 34:3 error "cold' attributes are not compatible"
//@ diag: 33:3 note "conflicting attribute is here"
//@ diag: 36:16 error "'cold' and 'hot' attributes are not compatible"
//@ diag: 35:16 note "conflicting attribute is here"
//@ diag: 37:38 error "hot' and 'cold' attributes are not compatible"
//@ diag: 37:16 note "conflicting attribute is here"
//@ diag: 39:23 error "'cold' and 'hot' attributes are not compatible"
//@ diag: 39:18 note "conflicting attribute is here"
//@ diag: 41:40 error "'cold' and 'hot' attributes are not compatible"
//@ diag: 41:35 note "conflicting attribute is here"
//@ diag: 46:22 error "always_destroy' and '"
//@ diag: 46:3 note "conflicting attribute is here"
//@ diag: 48:3 error "always_destroy' and '"
//@ diag: 47:3 note "conflicting attribute is here"
//@ diag: 52:3 error "always_destroy' and '"
//@ diag: 50:5 note "conflicting attribute is here"
//@ diag: 54:28 error "no_destroy' and '"
//@ diag: 54:5 note "conflicting attribute is here"

__attribute__((hot, cold)) void f1();
__attribute__((cold, hot)) void f2();
[[gnu::cold]] void f3();
[[gnu::hot]] void f3();
__attribute__((hot)) void f4();
__attribute__((cold)) void f4() {}
__attribute__((cold)) __attribute__((hot)) void f5();
struct S {
  __attribute__((hot, cold)) void m();
};
template <class T> __attribute__((hot, cold)) void t(T);
__attribute__((hot)) void ok1();
__attribute__((cold)) void ok2();
__attribute__((hot, hot)) void ok3();

[[clang::no_destroy, clang::always_destroy]] int v1;
[[clang::no_destroy]] extern int v2;
[[clang::always_destroy]] int v2;
struct T {
  [[clang::no_destroy]] static int m;
};
[[clang::always_destroy]] int T::m;
void g() {
  [[clang::always_destroy, clang::no_destroy]] static int l;
  (void)l;
}
[[clang::no_destroy]] int ok4;
[[clang::always_destroy]] int ok5;
