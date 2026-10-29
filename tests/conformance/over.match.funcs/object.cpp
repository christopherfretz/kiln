// [over.match.funcs]/4-5: the implicit object parameter of a member
// function is a reference to its cv-qualified class, an lvalue reference
// without a ref-qualifier or with &, an rvalue reference with &&; an
// rvalue object still binds to a non-ref-qualified member, and static
// members match any object.
//@ expect: compile-ok
template <class T, class U> struct same { static constexpr bool value = false; };
template <class T> struct same<T, T> { static constexpr bool value = true; };
struct S {
  char f();
  int f() const;
  long f() volatile;
  char g() &;
  int g() &&;
  char h(int);
  static int h(long);
  char k();
  int k(int = 0) const;
};
S make();
S &&xv();
void t(S &s, const S &cs, volatile S &vs) {
  static_assert(same<decltype(s.f()), char>::value);
  static_assert(same<decltype(cs.f()), int>::value);
  static_assert(same<decltype(vs.f()), long>::value);
  static_assert(same<decltype(make().f()), char>::value);
  static_assert(same<decltype(s.g()), char>::value);
  static_assert(same<decltype(xv().g()), int>::value);
  static_assert(same<decltype(make().g()), int>::value);
  static_assert(same<decltype(s.h(1)), char>::value);
  static_assert(same<decltype(s.h(1L)), int>::value);
  static_assert(same<decltype(cs.h(1L)), int>::value);
  static_assert(same<decltype(S::h(1L)), int>::value);
  static_assert(same<decltype(cs.k()), int>::value);
}
