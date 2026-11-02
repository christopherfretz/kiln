// [temp.constr.atomic]/3, [dcl.fct]/22, [temp.friend]/1: a friend function
// declaration with a placeholder parameter type (`friend R f(S, auto)`, with
// or without a type-constraint, defined in the class or not) declares an
// abbreviated friend function template. A call found by ADL from an atomic
// constraint deduces it, so after substitution the constraint is a
// non-dependent constant of type bool. This is libc++'s
// sized_sentinel_for<S, I> with a hidden-friend `operator-(C auto const&, S)`
// (range.iter.ops/types.h distance_apriori_sentinel).
//@ expect: run
//@ stdout: -3 3 7 9
extern "C" int printf(const char*, ...);

template <class T> concept Any = true;
template <class T> concept Small = sizeof(T) <= 8;

struct S {
  long n;
  friend constexpr long operator-(Any auto const&, S s) { return -s.n; }
  friend constexpr long operator-(S s, Any auto const&) { return s.n; }
  friend long g(S, auto const&);
};
long g(S s, auto const& x) { return s.n + sizeof(x); }
struct I {};

template <class Sp, class Ip>
concept Sized = requires(const Ip& i, const Sp& s) {
  { s - i } -> Small;
  { i - s } -> Small;
};
static_assert(Sized<S, I>);
static_assert(!Sized<I, I>);

template <class A, class B> concept Callable = sizeof(g(A{}, B{})) == sizeof(long);
static_assert(Callable<S, I>);

template <class T> struct W {
  T v;
  friend constexpr T get(W w, Small auto) { return w.v; }
};
template <class X> concept Gettable = requires(X x) { get(x, 0); };
static_assert(Gettable<W<int>>);
static_assert(get(W<int>{9}, 'c') == 9);

int main() {
  S s{3};
  I i;
  printf("%ld %ld %ld %d\n", i - s, s - i, g(S{6}, 'c'), get(W<int>{9}, 1));
}
