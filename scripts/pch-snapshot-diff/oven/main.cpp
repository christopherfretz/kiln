#include <cstdio>
// Uses of the prefix's evaluated state, and new evaluations of its
// compiled functions.
static_assert(pfx::fib22 == 17711);
static_assert(pfx::fib(23) == 28657);
static_assert(pfx::squares[63] == 3969);
static_assert(pfx::total_area(10) == 410);
static_assert(pfx::total_area(11) == 542);
static_assert(pfx::try_div(7, 0) == -1 && pfx::try_div(9, 3) == 3);
static_assert(pfx::active_int(5) == 5);
static_assert(pfx::pick(&pfx::Pair::a, {4, 5}) == 4);
static_assert(pfx::longest_word == 5);
static_assert(pfx::find(49) == 7 && !pfx::find(50));
static_assert(pfx::as_long(2.5) == 2 && pfx::as_long(7L) == 7);
static_assert(pfx::counted == 20100);
static_assert(pfx::Counter<201>::value == 20301);
constexpr int local_area = pfx::Square(6).area();
int main() {
  std::printf("%lu %d %d %zu\n", pfx::fib22, pfx::squares[9], local_area, pfx::longest_word);
  return pfx::total_area(3) == 18 ? 0 : 1;
}
