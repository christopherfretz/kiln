// [over.over]/5 (CWG2918): the functions whose constraints are not
// satisfied are eliminated, then a non-template more
// partial-ordering-constrained than another ([temp.constr.order]/7) beats
// it; with a placeholder target, [temp.deduct.call]/6 deduces from the
// members whose constraints are satisfied when they all give the same type.
// Found running GCC's C++ testsuite.
//@ expect: compile-ok
//@ requires: clang>=22
template <class T> concept Int = __is_same(T, int);
template <bool B> struct X {
  void f() requires B;
  void f() requires (!B);
  static constexpr int g(int) requires B { return 1; }
  static constexpr int g(int) requires (!B) { return 2; }
  void h(int) requires B;
  void h(int);
  static void n() noexcept requires B;
  static void n() requires (!B);
};
auto p1 = &X<true>::f;
static_assert(__is_same(decltype(p1), void (X<true>::*)()));
constexpr auto p2 = &X<false>::g;
static_assert(__is_same(decltype(p2), int (*const)(int)));
static_assert(p2(0) == 2);
auto p3 = X<true>::g;
static_assert(__is_same(decltype(p3), int (*)(int)));
auto &r3 = X<true>::g;
static_assert(__is_same(decltype(r3), int (&)(int)));
static_assert(r3(0) == 1);
auto p4 = &X<true>::h;
static_assert(__is_same(decltype(p4), void (X<true>::*)(int)));

// An explicit target: a pointer to member, a function pointer, a
// parameter, a cast, a noexcept target.
void (X<false>::*m1)() = &X<false>::f;
int (*f1)(int) = &X<true>::g;
void take(int (*)(int));
void t1() {
  take(&X<true>::g);
  take(X<false>::g);
}
auto c1 = static_cast<int (*)(int)>(&X<true>::g);
void (*nx)() noexcept = &X<true>::n;
auto nn = &X<true>::n;
static_assert(__is_same(decltype(nn), void (*)() noexcept));

// Function templates: constraints are part of their deduction.
template <class T> struct Y {
  template <class U> static int k(U) requires Int<T> { return 1; }
  template <class U> static int k(U) requires (!Int<T>) { return 2; }
};
int (*k1)(long) = &Y<int>::k;
int (*k2)(long) = &Y<char>::k<long>;
auto k3 = &Y<int>::k<long>;

// A template-id naming several specializations names the one whose
// template is more specialized (more constrained, [temp.func.order]/6).
template <class T> concept Class = __is_class(T);
template <class T> concept Empty = Class<T> && __is_empty(T);
template <Class T> constexpr int e(T) { return 1; }
template <Empty T> constexpr int e(T) { return 2; }
struct S0 {};
auto e1 = &e<S0>;
static_assert(__is_same(decltype(e1), int (*)(S0)));

template <bool B> void in_template() {
  auto p = &X<B>::f;
  static_assert(__is_same(decltype(p), void (X<B>::*)()));
  auto q = &X<true>::g;
  static_assert(__is_same(decltype(q), int (*)(int)));
}
template void in_template<false>();
