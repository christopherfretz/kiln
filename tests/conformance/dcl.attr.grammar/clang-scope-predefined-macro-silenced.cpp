// [dcl.attr.grammar]/4: the `__clang__` attribute-scope warning is in
// -Wignored-attributes, so -Wno-ignored-attributes silences it while the
// attribute keeps its effect; an unknown name under the scope still warns
// under -Wunknown-attributes (WP-889, WP-891). Verified against clang 19
// and 22.
//@ args: -Wno-ignored-attributes
//@ expect: compile-ok
//@ strict-diags
//@ diag: 14:3 warning "bogus' ignored"
//@ diag: 19:21 warning "temporary bound to local reference 'a' will be destroyed at the end of the full-expression"

[[__clang__::noinline]] void f1();
[[using __clang__: noinline]] void f2();
[[__clang__::bogus]] void f3();

const int &id(const int &x [[__clang__::lifetimebound]]);

void use() {
  const int &a = id(1);
  (void)a;
}
