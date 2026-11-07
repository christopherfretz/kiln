// [except.spec]/13, [temp.inst]/14: a function template specialization's
// exception specification is instantiated when it is needed, and naming
// the specialization needs it in any context, a template's too: inside a
// template body, a template parameter's type and a default template
// argument, `decltype(&h<true>)` is `void (*)() noexcept`. kiln resolved
// the deferred specification only outside templates, so the type lost
// `noexcept` there (a different type identity, mangling and NTTP type).
//@ expect: compile-ok
//@ strict-diags
//@ std: c++17
template <bool B> void h() noexcept(B) {}
void g() {}

template <class U> void k() {
  static_assert(__is_same(decltype(&h<true>), void (*)() noexcept));
  static_assert(__is_same(decltype(&h<false>), void (*)()));
  static_assert(noexcept(h<true>()) && !noexcept(h<false>()));
}

template <class U, decltype(&h<true>) P = nullptr> struct Q {};
static_assert(__is_same(Q<int>, Q<int, (void (*)() noexcept)nullptr>));

template <decltype(&h<true>) P> void w1() {}
template <decltype(&h<false>) P> void w2() {}
template <decltype(h<false>) *P> void w3() {}

template <class T> struct R {
  using F = decltype(&h<sizeof(T) == 1>);
};
static_assert(__is_same(R<char>::F, void (*)() noexcept));
static_assert(__is_same(R<int>::F, void (*)()));

void f() {
  k<int>();
  w1<&h<true>>();
  w2<&g>();
  w2<&h<true>>();
  w3<&g>();
}
