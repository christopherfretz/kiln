// [temp.names]/6: -Wno-error=missing-template-arg-list-after-template-kw
// demotes the error-by-default diagnostic to a warning.
//@ verify
//@ args: -Wno-error=missing-template-arg-list-after-template-kw

struct S { template <class U> void f(U); };
void m(S s) {
  s.template f(1); // expected-warning {{a template argument list is expected after a name prefixed by the template keyword}}
}
