// [temp.deduct.type]/5.1, [temp.spec.partial.match]/2, [temp.arg.template]:
// in `template<class T> struct Y<T::template B, T>` the template template
// argument `T::template B` is a non-deduced context: `T` is deduced from
// the second argument, and the first must then name the same template.
// kiln compared the dependent name with the argument's template during
// deduction, so the partial specialization never matched and the primary
// was chosen silently (found running GCC's C++ testsuite). Siblings:
// templates with default arguments and packs (P0522), a partial
// specialization that deduces from a nested template-id, a variable
// template, and through an enclosing template.
//@ expect: compile-ok
//@ strict-diags
//@ std: c++20
template <template <class...> class TT, class T>
struct Y {
  static constexpr int v = 0;
};
template <class T>
struct Y<T::template B, T> {
  static constexpr int v = 1;
};
template <class T>
struct Y<T::template P, T> {
  static constexpr int v = 2;
};

struct H1 {
  template <class>
  struct B {};
  template <class...>
  struct P {};
};
struct H2 {
  template <class, class = int>
  struct B {};
  template <class...>
  struct P {};
};
template <class>
struct Other {};

static_assert(Y<H1::B, H1>::v == 1);
static_assert(Y<H2::B, H2>::v == 1);
static_assert(Y<H1::P, H1>::v == 2);
static_assert(Y<H2::P, H2>::v == 2);
static_assert(Y<H1::B, H2>::v == 0);
static_assert(Y<Other, H1>::v == 0);

// A template template parameter with one parameter, given a template
// with a default argument (P0522).
template <template <class> class TT, class T>
struct R {
  static constexpr int v = 0;
};
template <class T>
struct R<T::template B, T> {
  static constexpr int v = 1;
};
static_assert(R<H2::B, H2>::v == 1);
static_assert(R<H1::B, H1>::v == 1);
static_assert(R<Other, H1>::v == 0);

// `T` deduced from the first argument, the nested template-id checked
// after substitution.
template <template <class> class>
struct W {};
template <class T, class U>
struct Z {
  static constexpr int v = 0;
};
template <class T>
struct Z<T, W<T::template B>> {
  static constexpr int v = 1;
};
static_assert(Z<H1, W<H1::B>>::v == 1);
static_assert(Z<H1, W<Other>>::v == 0);

template <template <class...> class TT, class T>
constexpr int vy = 0;
template <class T>
constexpr int vy<T::template B, T> = 1;
static_assert(vy<H1::B, H1> == 1);
static_assert(vy<Other, H1> == 0);

// Named inside another template.
template <class T>
struct C {
  static constexpr int v = Y<T::template B, H1>::v;
};
static_assert(C<H1>::v == 1);
static_assert(C<H2>::v == 0);
