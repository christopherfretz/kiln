// clang's `enable_if` attribute (clang AttrDocs "enable_if";
// [over.match.viable], [over.match.best] as clang extends them): a candidate
// is viable only if each condition, evaluated as a constant expression with
// the call's converted arguments for the parameters, is true; of two viable
// candidates, the one whose conditions extend the other's wins; functions
// whose conditions differ are overloads, not redeclarations.
// Verified against clang 19 and clang 22.
//@ expect: compile-ok
//@ strict-diags

// A failed condition falls back to the other overload.
constexpr int f(int n) __attribute__((enable_if(n == 0, "zero"))) { return 1; }
constexpr int f(double) { return 2; }
static_assert(f(0) == 1);
static_assert(f(1) == 2);

// The attribute makes `h(int)` another overload; with it, it is better.
constexpr int h(int) { return 3; }
constexpr int h(int n) __attribute__((enable_if(n > 0, "positive"))) { return 4; }
static_assert(h(1) == 4);
static_assert(h(-1) == 3);

// A proper superset of conditions wins; repeated attributes compose.
constexpr int a(int n) __attribute__((enable_if(n > 0, "pos"))) { return 1; }
constexpr int a(int n) __attribute__((enable_if(n > 0, "pos"), enable_if(n > 10, "big"))) {
  return 2;
}
static_assert(a(5) == 1);
static_assert(a(50) == 2);

// A redeclaration repeats the conditions.
constexpr int r(int n) __attribute__((enable_if(n != 0, "nonzero")));
constexpr int r(int n) __attribute__((enable_if(n != 0, "nonzero"))) { return n; }
static_assert(r(4) == 4);

// `__builtin_constant_p` of a parameter: a constant argument only.
constexpr int k(int n) __attribute__((enable_if(__builtin_constant_p(n), "constant"))) {
  return 1;
}
constexpr int k(int) { return 2; }
static_assert(k(3) == 1);
int runtime(int v) { return k(v); }

// A function template's condition is evaluated after deduction.
template <class T> constexpr int t(T x) __attribute__((enable_if(x == 0, ""))) { return 7; }
template <class T> constexpr int t(T) { return 8; }
static_assert(t(0) == 7);
static_assert(t(0L) == 7);
static_assert(t(2) == 8);

// An omitted argument is its default argument.
constexpr int d(int n = 3) __attribute__((enable_if(n == 3, "three"))) { return n; }
static_assert(d() == 3);

// The literal-zero idiom: a consteval converting constructor viable only for
// a constant 0 (libc++ <__compare/ordering.h>).
struct Zero {
  template <class T>
  consteval Zero(T z) __attribute__((enable_if(z == 0, "only literal 0"))) {}
};
struct Ord {
  int v;
  friend constexpr bool operator<(Ord o, Zero) { return o.v < 0; }
};
static_assert(Ord{-1} < 0);
template <class T> constexpr bool lt_zero = requires(T t) { t < 0; };
template <class T> constexpr bool lt_one = requires(T t) { t < 1; };
static_assert(lt_zero<Ord> && !lt_one<Ord>);

// A member function and a constructor.
struct S {
  int q = 0;
  constexpr int m(int n) const __attribute__((enable_if(n == 1, "one"))) { return 10; }
  constexpr int m(long) const { return 20; }
};
static_assert(S{}.m(1) == 10);
static_assert(S{}.m(2) == 20);
