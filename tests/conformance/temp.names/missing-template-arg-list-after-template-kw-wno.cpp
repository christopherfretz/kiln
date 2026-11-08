// [temp.names]/6: -Wno-missing-template-arg-list-after-template-kw turns
// clang's error-by-default diagnostic off; the program is then accepted.
//@ verify
//@ args: -Wno-missing-template-arg-list-after-template-kw
// expected-no-diagnostics

struct S { template <class U> void f(U); };
template <class T> void dep(T t) { t.template f(1); }
void m(S s) { s.template f(1); dep(s); }
