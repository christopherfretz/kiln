// [temp.names]/6 (P1787): a name prefixed by the `template` keyword must
// be followed by a template argument list unless it names a class or
// alias template used as a type, or a template template argument. clang
// diagnoses the omission with -Wmissing-template-arg-list-after-template-kw,
// an error by default, in both dependent and non-dependent contexts.
//@ verify

template <class T> struct A {
  template <class U> static void f(U);
  template <class U> static constexpr int v = 0; // expected-note {{template is declared here}}
  template <class U> struct N {};
};
template <class T> void g() {
  auto p = &A<T>::template f; // expected-error {{a template argument list is expected after a name prefixed by the template keyword}}
  int x = A<T>::template v; // expected-error {{a template argument list is expected after a name prefixed by the template keyword}}
  typename A<T>::template N<int> n;
}
void h() {
  auto q = &A<int>::template f<int>;
  int y = A<int>::template v<int>;
  void (*fp)(int) = &A<int>::template f; // expected-error {{a template argument list is expected after a name prefixed by the template keyword}}
  int z = A<int>::template v; // expected-error {{a template argument list is expected after a name prefixed by the template keyword}} expected-error {{requires template arguments}}
}

struct S { template <class U> void f(U); };
template <class T> void dep(T t) {
  t.template f; // expected-error {{a template argument list is expected after a name prefixed by the template keyword}}
  t.template f(1); // expected-error {{a template argument list is expected after a name prefixed by the template keyword}}
}
void m(S s) {
  s.template f(1); // expected-error {{a template argument list is expected after a name prefixed by the template keyword}}
  s.template f<int>(1);
}
template <class T> struct B {
  template <class U> static void f(U);
  void m() { B::template f(1); } // expected-error {{a template argument list is expected after a name prefixed by the template keyword}}
};
