// [except.spec]/13, [temp.variadic]/5: the deferred noexcept-specifiers of
// two member function templates whose template and function parameter
// lists have the same shape each see their own parameters. Here `U` is a
// type in `f` and a pack in `g`. The scopes of `f` and `g` are reinstated
// in turn with no declaration in between, so a name-classification cache
// keyed on the scope stack's shape alone would answer `g`'s `U` with
// `f`'s (WP-919; libc++ 19 `tuple`'s
// `noexcept(_And<is_nothrow_constructible<_Tp, _Up>...>::value)`).
//@ expect: compile-ok
template <bool... B> struct all { static constexpr bool value = (B && ...); };

struct S {
  template <class T, class U> static void f(T t, U u) noexcept(sizeof(U) > 0) {}
  template <class T, class... U>
  static void g(T t, U... u) noexcept(all<(sizeof(U) > 0)...>::value) {}
  template <class T, class... U>
  static void h(T t, U... u) noexcept(all<(sizeof(U) > 1)...>::value) {}
};

static_assert(noexcept(S::f(1, 2)));
static_assert(noexcept(S::g(1, 2, 3)));
static_assert(!noexcept(S::h(1, 'a', 3)));
static_assert(noexcept(S::h(1, 2L, 3)));

int main() {}
