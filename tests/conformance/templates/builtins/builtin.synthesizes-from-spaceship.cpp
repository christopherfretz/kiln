// __builtin_{lt,le,gt,ge}_synthesizes_from_spaceship(T, U) (clang 22;
// WP-1740): whether `t op u` on operands of types T and U (an lvalue for
// an lvalue reference, an xvalue for an rvalue reference, else a
// prvalue) resolves to a rewritten operator<=> candidate
// ([over.match.oper]/3.4, /8), reversed ones included. A built-in
// comparison, a direct operator<, and an ill-formed or ambiguous
// comparison are all false. libc++ 22's __default_three_way_comparator
// asks it. Oracle-checked by hand with clang++-22 (22.1.8): same answers.
//@ requires: clang>=22
//@ std: c++26
#include <compare>

struct S {
  int v;
  auto operator<=>(const S &) const = default;
};
struct L {
  int v;
  bool operator<(const L &) const;
};
struct N {};
struct M {
  int v;
  std::strong_ordering operator<=>(int) const;
};
struct R {
  int v;
  std::strong_ordering operator<=>(const R &) &;
};

static_assert(__builtin_lt_synthesizes_from_spaceship(const S &, const S &));
static_assert(__builtin_le_synthesizes_from_spaceship(const S &, const S &));
static_assert(__builtin_gt_synthesizes_from_spaceship(S, S));
static_assert(__builtin_ge_synthesizes_from_spaceship(S &&, S));
static_assert(!__builtin_lt_synthesizes_from_spaceship(int, int));
static_assert(!__builtin_lt_synthesizes_from_spaceship(int *, int *));
static_assert(!__builtin_lt_synthesizes_from_spaceship(L, L));
static_assert(!__builtin_lt_synthesizes_from_spaceship(N, N));
static_assert(__builtin_lt_synthesizes_from_spaceship(M, int));
static_assert(__builtin_gt_synthesizes_from_spaceship(int, M));
static_assert(!__builtin_lt_synthesizes_from_spaceship(M, M));
static_assert(!__builtin_lt_synthesizes_from_spaceship(void, int));
static_assert(__builtin_lt_synthesizes_from_spaceship(R &, const R &));
static_assert(!__builtin_lt_synthesizes_from_spaceship(R, R));

template <class T>
constexpr bool synth = __builtin_lt_synthesizes_from_spaceship(const T &, const T &);
static_assert(synth<S> && !synth<L> && !synth<double>);
