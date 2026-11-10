// [expr.prim.lambda.general] (GNU extension, as in libc++ simd_utils.h):
// GNU attributes may appear between the lambda's parameter list and its
// lambda-specifier-seq / noexcept-specifier; they appertain to the call
// operator.
//@ skip(gcc): clang's GNU-attribute placement before the lambda-specifiers, which GCC 16.1 rejects
auto a = [](int x) __attribute__((__always_inline__)) noexcept { return x; };
static_assert(noexcept(a(1)));
auto c = [](int x) __attribute__((__always_inline__)) constexpr { return x; };
static_assert(c(2) == 2);
auto m = [n = 0]() __attribute__((__always_inline__)) mutable noexcept { return ++n; };
static_assert(noexcept(m()));
template <class T> auto g(T t) {
  return [t]() __attribute__((__always_inline__)) __attribute__((__unused__)) mutable noexcept { return t; };
}
auto l = g(1);
static_assert(noexcept(l()));
static_assert(__is_same(decltype(g(1)()), int));
