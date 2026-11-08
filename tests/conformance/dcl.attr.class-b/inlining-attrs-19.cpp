// The inlining and call attributes' subjects as clang 19 checks them,
// the statement forms of nomerge / noinline / always_inline (ignored
// without a call), the always_inline / not_tail_called
// MutualExclusions, and __has_attribute (spec/landed/codegen/WP-813.md).
// [dcl.attr.grammar]/5. WP-813.
//@ verify
//@ requires: clang<22
__attribute__((flatten)) int v1; // expected-error {{'flatten' attribute only applies to functions}}
__attribute__((flatten(1))) void f1(); // expected-error {{'flatten' attribute takes no arguments}}
__attribute__((leaf)) int v2; // expected-warning {{'leaf' attribute only applies to functions}}
__attribute__((nomerge)) int v3; // expected-warning {{'nomerge' attribute is ignored because 'v3' is not a function pointer}}
__attribute__((not_tail_called)) int v4; // expected-warning {{'not_tail_called' attribute only applies to functions}}
__attribute__((noduplicate)) int v5; // expected-warning {{'noduplicate' attribute only applies to functions}}
__attribute__((convergent)) int v6; // expected-warning {{'convergent' attribute only applies to functions}}
__attribute__((nouwtable)) int v7; // expected-warning {{'nouwtable' attribute only applies to functions and function pointers}}
struct A { __attribute__((not_tail_called)) virtual void vm(); __attribute__((always_inline, not_tail_called)) void m(); }; // expected-error {{'not_tail_called' and 'always_inline' attributes are not compatible}} expected-note {{conflicting attribute is here}}
void g();
void s() {
  [[clang::nomerge]] int x = 0; // expected-warning {{'nomerge' attribute is ignored because 'x' is not a function pointer}}
  [[clang::nomerge(1)]] g(); // expected-error {{'nomerge' attribute takes no arguments}}
  [[clang::noinline]] x = 1; // expected-warning {{'noinline' attribute is ignored because there exists no call expression inside the statement}}
  [[clang::always_inline]] g();
  [[clang::nomerge]] g();
  [[clang::noinline]] g();
  (void)x;
}
__attribute__((not_tail_called, always_inline)) void nt1(); // expected-error {{'always_inline' and 'not_tail_called' attributes are not compatible}} expected-note {{conflicting attribute is here}}
__attribute__((always_inline)) void nt2(); // expected-note {{conflicting attribute is here}}
__attribute__((not_tail_called)) void nt2(); // expected-error {{'not_tail_called' and 'always_inline' attributes are not compatible}}
__attribute__((not_tail_called)) void nt3(); // expected-note {{conflicting attribute is here}}
__attribute__((always_inline)) void nt3(); // expected-error {{'always_inline' and 'not_tail_called' attributes are not compatible}}
[[gnu::always_inline, clang::not_tail_called]] void nt4(); // expected-error {{'not_tail_called' and 'always_inline' attributes are not compatible}} expected-note {{conflicting attribute is here}}
#if !__has_attribute(flatten) || !__has_attribute(leaf) || !__has_attribute(nomerge) || !__has_attribute(not_tail_called) || !__has_attribute(noduplicate) || !__has_attribute(convergent) || !__has_attribute(nouwtable) || !__has_cpp_attribute(clang::nomerge) || !__has_cpp_attribute(gnu::flatten)
#error "not reported"
#endif

