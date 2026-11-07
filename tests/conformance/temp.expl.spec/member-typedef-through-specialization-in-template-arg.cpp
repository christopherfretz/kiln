// [temp.expl.spec]/1, [temp.names]/1: `traits<clk>::spec::fine_clock`
// names a member of the explicit specialization `traits<clk>`, not of the
// primary template, whose `spec` is a different (here dependent) type. The
// qualified name parses as a type in a template argument list too: the
// lookahead that decides type-id vs expression resolves `spec` through the
// specialization. The shape of folly's `clock_traits<steady_clock>::spec`.
// WP-726.
//@ std: c++20
//@ expect: run
//@ stdout: 1 1 1
#include <cstdio>
#include <type_traits>
struct clk {};
struct sp { using fine_clock = clk; };
template <class C> struct traits { using spec = typename C::spec_type; };
template <> struct traits<clk> { using spec = sp; };
template <class C> struct traits2 { using spec = void; };
template <> struct traits2<clk> { using spec = sp; };
template <class T> constexpr bool is_clk = std::is_same_v<T, clk>;
using X = traits<clk>::spec::fine_clock;
int main() {
  constexpr bool a = is_clk<traits<clk>::spec::fine_clock>;
  constexpr bool b = std::is_same_v<traits2<clk>::spec::fine_clock, clk>;
  std::printf("%d %d %d\n", a, b, is_clk<X>);
}
