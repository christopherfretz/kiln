// [dcl.attr.grammar]/4: clang treats `_Clang` as its attribute namespace, and
// accepts `__clang__` too (it is a predefined macro expanding to 1) with an
// -Wignored-attributes warning and a fix-it to `_Clang`, also as the
// attribute-using-prefix namespace. The attribute still takes effect
// (WP-889, WP-891; spec/parse.md). Verified against clang 19 and 22.
//@ expect: compile-ok
//@ strict-diags
//@ diag: 20:3 warning "'__clang__' is a predefined macro name, not an attribute scope specifier; did you mean '_Clang' instead?"
//@ diag: 22:9 warning "'__clang__' is a predefined macro name, not an attribute scope specifier; did you mean '_Clang' instead?"
//@ diag: 24:3 warning "'__clang__' is a predefined macro name, not an attribute scope specifier; did you mean '_Clang' instead?"
//@ diag: 24:3 warning "bogus' ignored"
//@ diag: 25:3 warning "bogus' ignored"
//@ diag: 26:3 warning "'__clang__' is a predefined macro name, not an attribute scope specifier; did you mean '_Clang' instead?"
//@ diag: 29:31 warning "'__clang__' is a predefined macro name, not an attribute scope specifier; did you mean '_Clang' instead?"
//@ diag: 30:37 warning "'__clang__' is a predefined macro name, not an attribute scope specifier; did you mean '_Clang' instead?"
//@ diag: 33:22 warning "temporary bound to local reference 'a' will be destroyed at the end of the full-expression"
//@ diag: 34:22 warning "temporary bound to local reference 'b' will be destroyed at the end of the full-expression"
//@ diag: 35:22 warning "temporary bound to local reference 'c' will be destroyed at the end of the full-expression"

[[__clang__::noinline]] void f1();
[[_Clang::noinline]] void f2();
[[using __clang__: noinline]] void f3();
[[using _Clang: noinline]] void f4();
[[__clang__::bogus]] void f5();
[[_Clang::bogus]] void f6();
[[__clang__::noinline, _Clang::noinline]] void f7();

const int &id1(const int &x [[_Clang::lifetimebound]]);
const int &id2(const int &x [[__clang__::lifetimebound]]);
const int &id3(const int &x [[using __clang__: lifetimebound]]);

void use() {
  const int &a = id1(1);
  const int &b = id2(2);
  const int &c = id3(3);
  (void)a; (void)b; (void)c;
}
