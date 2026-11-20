// [temp.constr.atomic]/3, [temp.constr.decl]/3, [expr.prim.lambda.closure]/3:
// a lambda in a generic lambda in a template is rebuilt twice, when the
// template is instantiated (the generic lambda's parameters stay template
// parameters) and when the generic call operator is; its constraints are
// those of the lambda as written, checked with the arguments of every
// enclosing level: the template's, the generic lambda's and its own. The
// same holds for a member of a local class there. Found running GCC's C++
// testsuite (a requires-expression naming all three levels).
//@ oracle: gcc
template <class T> concept C = sizeof(T) > 1;
template <class F, class A> concept Callable = requires(F f, A x) { f(x); };
template <class F> concept Nullary = requires(F f) { f(); };

template <class T>
auto a = [] <class U> (U) {
  return [] <class V> (V) requires requires (T t, U u, V v) { t + u + v; } { return 1; };
};
static_assert(a<int>(0)(0) == 1);
static_assert(!Callable<decltype(a<int>(0)), decltype(nullptr)>);

template <class T>
struct A {
  static constexpr auto f() {
    return [] <class U> (U) {
      return [] <class V> (V) requires requires (T t, U u, V v) { t + u + v; } { return 2; };
    };
  }
  template <class W>
  static constexpr auto g() {
    return [] (auto u) {
      return [] <class V> (V) requires C<W> && C<decltype(u)> && C<V> { return 3; };
    };
  }
};
static_assert(A<int>::f()(0)(0) == 2);
static_assert(!Callable<decltype(A<int>::f()(0)), decltype(nullptr)>);
static_assert(A<int>::g<long>()(0)(0L) == 3);
static_assert(!Callable<decltype(A<int>::g<long>()(0)), char>);
static_assert(!Callable<decltype(A<int>::g<char>()(0)), long>);

// Three generic levels, the innermost lambda not generic.
template <class T>
auto b = [] <class U> (U) {
  return [] (auto v) {
    return [] () requires C<T> && C<U> && C<decltype(v)> { return 4; };
  };
};
static_assert(b<int>(0)(0)() == 4);

template <class T>
auto three = [] (auto) { return [] (auto) { return [] <class V> (V) requires C<V> { return 5; }; }; };
static_assert(three<int>(0)(0)(0) == 5);
static_assert(!Callable<decltype(three<int>(0)(0)), char>);

// A member of a local class: not checked while the generic lambda is a
// pattern, checked with every level once it is instantiated.
template <class T>
auto local = [] <class U> (U) {
  struct L { static constexpr int g() requires C<U> && C<T> { return 6; } };
  return L::g();
};
static_assert(local<int>(0) == 6);

template <class T>
void fn() {
  auto l = [] <class U> (U) { return [] <class V> (V) requires (sizeof(T) + sizeof(U) + sizeof(V) > 3) { return 7; }; };
  static_assert(l(0)(0) == 7);
}
template void fn<int>();

// A constraint naming a parameter of an enclosing generic lambda in an
// unevaluated operand names the enclosing specialization's parameter.
auto g = [] <class U> (U) { return [] (auto v) { return [] () requires C<decltype(v)> { return 8; }; }; };
static_assert(g(0)(0)() == 8);
static_assert(Nullary<decltype(g(0)(0))>);
static_assert(!Nullary<decltype(g(0)('c'))>);
template <class T>
void fn2() {
  auto l = [] (auto a, auto... r) { return [] <class V> (V) requires C<decltype(a)> && (sizeof...(r) == 2) { return 9; }; };
  static_assert(l(1, 2, 3)(2) == 9);
}
template void fn2<int>();
int main() {}
