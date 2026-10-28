//@ mode: pp
// Macro-engine stress in the style of Boost.PP and P99: recursion through
// deferred expansion and nested EVAL rescans, `__VA_OPT__`, token pasting and
// stringizing. Every pass is a fresh rescan of large argument lists; the file
// re-includes itself (8 x 4 passes) so the total is well above timer noise.
// The output is valid C++ (one table per pass).
#if !defined(REP_L1)
#define REP_L1

#define REP_EMPTY()
#define REP_DEFER(id) id REP_EMPTY()
#define REP_EVAL(...) REP_EVAL4(REP_EVAL4(REP_EVAL4(__VA_ARGS__)))
#define REP_EVAL4(...) REP_EVAL3(REP_EVAL3(REP_EVAL3(__VA_ARGS__)))
#define REP_EVAL3(...) REP_EVAL2(REP_EVAL2(REP_EVAL2(__VA_ARGS__)))
#define REP_EVAL2(...) REP_EVAL1(REP_EVAL1(REP_EVAL1(__VA_ARGS__)))
#define REP_EVAL1(...) __VA_ARGS__
// A second, smaller family for nested use: a macro is not re-expanded inside
// its own expansion, so REP_ROW cannot use REP_EVAL under REP_EVAL.
#define REP_IN_EVAL(...) REP_IN_EVAL3(REP_IN_EVAL3(REP_IN_EVAL3(__VA_ARGS__)))
#define REP_IN_EVAL3(...) REP_IN_EVAL2(REP_IN_EVAL2(REP_IN_EVAL2(__VA_ARGS__)))
#define REP_IN_EVAL2(...) REP_IN_EVAL1(REP_IN_EVAL1(REP_IN_EVAL1(__VA_ARGS__)))
#define REP_IN_EVAL1(...) __VA_ARGS__

// REP_MAP(f, a, b, ...) -> f(a) f(b) ...  (the outer macro expands its
// arguments, so a list macro becomes the elements)
#define REP_MAP(f, ...) REP_MAP_R(f, __VA_ARGS__)
#define REP_MAP_R(f, x, ...) f(x) __VA_OPT__(REP_DEFER(REP_MAP_I)()(f, __VA_ARGS__))
#define REP_MAP_I() REP_MAP_R
// REP_MAP2(f, ctx, a, ...) -> f(ctx, a) f(ctx, b) ...
#define REP_MAP2(f, c, ...) REP_MAP2_R(f, c, __VA_ARGS__)
#define REP_MAP2_R(f, c, x, ...) f(c, x) __VA_OPT__(REP_DEFER(REP_MAP2_I)()(f, c, __VA_ARGS__))
#define REP_MAP2_I() REP_MAP2_R
// REP_LIST(f, a, b, ...) -> f(a), f(b), ...
#define REP_LIST(f, ...) REP_LIST_R(f, __VA_ARGS__)
#define REP_LIST_R(f, x, ...) f(x) __VA_OPT__(, REP_DEFER(REP_LIST_I)()(f, __VA_ARGS__))
#define REP_LIST_I() REP_LIST_R

#define REP_CAT(a, b) REP_CAT_I(a, b)
#define REP_CAT_I(a, b) a##b
#define REP_STR(x) REP_STR_I(x)
#define REP_STR_I(x) #x

#define REP_DIGITS 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, a, b, c, d, e, f
#define REP_NAMES                                                                  \
  alpha, bravo, charlie, delta, echo, foxtrot, golf, hotel, india, juliett, kilo,  \
      lima, mike, november, oscar, papa, quebec, romeo, sierra, tango, uniform,    \
      victor, whiskey, xray, yankee, zulu

#define REP_FIELD(n) int REP_CAT(n, _value) = sizeof(REP_STR(n));
#define REP_NAME(n) REP_STR(REP_CAT(REP_PASS, n))
#define REP_CELL(n, d) REP_CAT(REP_CAT(n, _), d),
#define REP_ROW(n) REP_IN_EVAL(REP_MAP2(REP_CELL, n, REP_DIGITS))
#define REP_CASE(n)                                                                \
  case REP_CAT(n, _0):                                                             \
    return REP_STR(n);

#define REP_L2_DIGIT 0
#include "repetition.cpp"
#undef REP_L2_DIGIT
#define REP_L2_DIGIT 1
#include "repetition.cpp"
#undef REP_L2_DIGIT
#define REP_L2_DIGIT 2
#include "repetition.cpp"
#undef REP_L2_DIGIT
#define REP_L2_DIGIT 3
#include "repetition.cpp"
#undef REP_L2_DIGIT
#define REP_L2_DIGIT 4
#include "repetition.cpp"
#undef REP_L2_DIGIT
#define REP_L2_DIGIT 5
#include "repetition.cpp"
#undef REP_L2_DIGIT
#define REP_L2_DIGIT 6
#include "repetition.cpp"
#undef REP_L2_DIGIT
#define REP_L2_DIGIT 7
#include "repetition.cpp"
#undef REP_L2_DIGIT

int main() { return 0; }

#elif !defined(REP_L2)
#define REP_L2

#define REP_L3_DIGIT 0
#include "repetition.cpp"
#undef REP_L3_DIGIT
#define REP_L3_DIGIT 1
#include "repetition.cpp"
#undef REP_L3_DIGIT
#define REP_L3_DIGIT 2
#include "repetition.cpp"
#undef REP_L3_DIGIT
#define REP_L3_DIGIT 3
#include "repetition.cpp"
#undef REP_L3_DIGIT

#undef REP_L2
#else
// One pass: a namespace per (L2, L3) digit pair.
#define REP_PASS REP_CAT(REP_CAT(p, REP_L2_DIGIT), REP_L3_DIGIT)
namespace REP_PASS {
struct Fields {
  REP_EVAL(REP_MAP(REP_FIELD, REP_NAMES))
};
inline constexpr const char* names[] = {REP_EVAL(REP_LIST(REP_NAME, REP_NAMES))};
enum Cell { REP_EVAL(REP_MAP(REP_ROW, REP_NAMES)) };
inline constexpr const char* name_of(Cell c) {
  switch (c) {
    REP_EVAL(REP_MAP(REP_CASE, REP_NAMES))
    default:
      return "?";
  }
}
}  // namespace REP_PASS
#undef REP_PASS
#endif
