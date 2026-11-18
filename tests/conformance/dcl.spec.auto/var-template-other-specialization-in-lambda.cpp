// [dcl.spec.auto.general]/11, [temp.variable]: a variable template with a
// deduced type may name a DIFFERENT specialization of itself in a lambda
// in its initializer (a generic lambda's body, a non-generic lambda's body
// in a discarded branch, a pack peeled one element per specialization):
// that specialization's type is deduced on its own. Naming the SAME
// specialization there (g<I> in g<I>'s lambda) names a variable whose type
// is not deduced yet: clang 22 and kiln reject it, GCC 16.1 accepts it in a
// generic lambda's body (found running GCC's C++ testsuite).
//@ expect: compile-ok
//@ std: c++20
template <int I>
constexpr auto down = [](auto x) {
  if constexpr (I == 0)
    return x;
  else
    return down<I - 1>(x);
};
static_assert(down<3>(5) == 5);

template <int I>
constexpr auto plain = [](int x) -> int {
  if constexpr (I == 0)
    return x;
  else
    return plain<I - 1>(x + 1);
};
static_assert(plain<2>(5) == 7);

template <int I, int... Is>
constexpr auto peel = [](auto x) {
  if constexpr (sizeof...(Is) == 0)
    return x + I;
  else
    return peel<Is...>(x) + I;
};
static_assert(peel<1, 2, 3>(10) == 16);
