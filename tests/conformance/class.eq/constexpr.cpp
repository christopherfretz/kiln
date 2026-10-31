// [class.eq]/2-3: a defaulted `==` compares the base class subobjects,
// then the members in declaration order (arrays element-wise), with
// `&&` short-circuiting; an empty class compares equal. Constant
// evaluated in the Oven.
//@ expect: compile-ok
//@ strict-diags
struct Empty {
  constexpr bool operator==(const Empty&) const = default;
};
static_assert(Empty{} == Empty{});
struct B { int b; constexpr bool operator==(const B&) const = default; };
struct S : B {
  int a[3];
  char c;
  constexpr bool operator==(const S&) const = default;
};
static_assert(S{{1}, {1, 2, 3}, 'x'} == S{{1}, {1, 2, 3}, 'x'});
static_assert(S{{1}, {1, 2, 3}, 'x'} != S{{2}, {1, 2, 3}, 'x'});
static_assert(S{{1}, {1, 2, 4}, 'x'} != S{{1}, {1, 2, 3}, 'x'});
static_assert(S{{1}, {1, 2, 3}, 'y'} != S{{1}, {1, 2, 3}, 'x'});
// Short-circuiting: the second member's comparison is not evaluated
// once the first differs.
struct Trap {
  int v;
  constexpr bool operator==(const Trap& o) const {
    if (v < 0) throw 0;
    return v == o.v;
  }
};
struct T {
  int k;
  Trap t;
  constexpr bool operator==(const T&) const = default;
};
static_assert(T{1, {-1}} != T{2, {-1}});
// Members of class type use their own `==`, possibly a non-member.
struct N { int n; };
constexpr bool operator==(const N& x, const N& y) { return x.n % 10 == y.n % 10; }
struct U {
  N n;
  friend constexpr bool operator==(const U&, const U&) = default;
};
static_assert(U{{13}} == U{{3}});
// Nested arrays and a by-value parameter.
struct V {
  int m[2][2];
  friend constexpr bool operator==(V, V) = default;
};
static_assert(V{{{1, 2}, {3, 4}}} == V{{{1, 2}, {3, 4}}});
static_assert(V{{{1, 2}, {3, 5}}} != V{{{1, 2}, {3, 4}}});
// A floating member: NaN is not equal to itself.
struct F { double d; constexpr bool operator==(const F&) const = default; };
static_assert(F{__builtin_nan("")} != F{__builtin_nan("")});
static_assert(F{0.0} == F{-0.0});
