// [dcl.pre]/12, [expr.const]/10 (WP-143): failed static_asserts on
// constant expressions, located through -verify's @+N, @-N, absolute
// @line and @#marker forms and a repeat count.
//@ verify
template <int N> struct Fact { static constexpr int value = N * Fact<N - 1>::value; };
template <> struct Fact<0> { static constexpr int value = 1; };
static_assert(Fact<5>::value == 120);
// expected-error@+1 {{static assertion failed due to requirement 'Fact<3>::value == 7'}}
static_assert(Fact<3>::value == 7);
// expected-note@-1 {{expression evaluates to '6 == 7'}}
static_assert(sizeof(int) == 3); // #three
// expected-error@#three {{static assertion failed}}
// expected-note@#three {{expression evaluates to '4 == 3'}}
// expected-error@19 2 {{static assertion failed}}
// (a call is not decomposed: no "evaluates to" note on line 19)
constexpr bool f(int x) { return x > 0; }

// The next line is line 19:
static_assert(f(-1)); static_assert(f(-2));
