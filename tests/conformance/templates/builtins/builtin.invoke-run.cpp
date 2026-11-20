// __builtin_invoke(f, t1, ..., tN) (clang 21+; WP-1740) is the
// expression INVOKE(f, t1, ..., tN) denotes ([func.require]/1), which
// replaces the call: (t1.*f)(t2, ...) for a pointer to member function
// when t1's class is the member's class or derived from it,
// (t1.get().*f)(...) for a std::reference_wrapper, ((*t1).*f)(...)
// otherwise; t1.*f for a pointer to data member; else f(t1, ..., tN). It
// is a constant expression when that expression is, and its type,
// value category and noexcept are that expression's. libc++ 22's
// std::__invoke is built on it. Checked against clang++-22.
//@ requires: clang>=22
//@ std: c++26
//@ expect: run
//@ stdout: 42 43 7
#include <cstdio>
#include <functional>

struct S {
  int v;
  constexpr int get(int k) const { return v + k; }
  int &ref() { return v; }
};
struct D : S {};
constexpr int add(int a, int b) { return a + b; }

static_assert(__builtin_invoke(add, 1, 2) == 3);
static_assert(__builtin_invoke([](int x) { return x * 2; }, 4) == 8);
constexpr S s{5};
static_assert(__builtin_invoke(&S::get, s, 1) == 6);
static_assert(__builtin_invoke(&S::get, &s, 2) == 7);
static_assert(__builtin_invoke(&S::v, s) == 5);
static_assert(__builtin_invoke(&S::v, &s) == 5);
constexpr D d{{9}};
static_assert(__builtin_invoke(&S::get, d, 1) == 10);
static_assert(__is_same(decltype(__builtin_invoke(&S::v, s)), const int &));
static_assert(__is_same(decltype(__builtin_invoke(&S::v, S{1})), int &&));
static_assert(noexcept(__builtin_invoke(&S::v, s)));
static_assert(!noexcept(__builtin_invoke(add, 1, 2)));

template <class F, class... A>
constexpr decltype(auto) call(F &&f, A &&...a) {
  return __builtin_invoke(static_cast<F &&>(f), static_cast<A &&>(a)...);
}
static_assert(call(add, 3, 4) == 7);
static_assert(call(&S::get, s, 3) == 8);

template <class F, class = decltype(__builtin_invoke(F{}, 1))>
constexpr bool callable(int) { return true; }
template <class F> constexpr bool callable(long) { return false; }
static_assert(callable<int (*)(int)>(0) && !callable<int (*)()>(0));

int main() {
  S m{1};
  std::reference_wrapper<S> r(m);
  __builtin_invoke(&S::ref, r) = 41;
  int x = __builtin_invoke(&S::get, r, 1);
  int y = std::invoke(&S::get, m, 2);
  int z = call(add, 3, 4);
  std::printf("%d %d %d\n", x, y, z);
}
