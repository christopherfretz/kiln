// clang extension (GNU): a conditional whose condition is
// `__builtin_constant_p(e)` is evaluated by folding, so
// `__builtin_constant_p(E) ? E : E` is a constant expression even where `E`
// (here a reinterpret_cast, [expr.const]/5.15) is not. Boost.HOF's
// `BOOST_HOF_CONST_FOLD` (static lambdas, `constexpr_deduce_unique`) relies
// on it; clang accepts this and GCC does not.
//@ expect: run
//@ oracle: clang
//@ stdout: 1
extern "C" int printf(const char*, ...);
struct Tag {};
struct L { int operator()(int i) const { return i + 1; } };
constexpr Tag tag{};
#define R reinterpret_cast<const L&>(tag)
constexpr const L& f = __builtin_constant_p(R) ? R : R;

int main() { printf("%d\n", (int)(&f == reinterpret_cast<const L*>(&tag))); }
