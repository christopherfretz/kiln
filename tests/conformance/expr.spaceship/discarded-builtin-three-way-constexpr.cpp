// [expr.spaceship]/4, /8: a built-in three-way comparison is a prvalue of a
// comparison category type; [expr.context]/2: as a discarded-value
// expression its result object is a temporary. Its evaluation in a constant
// expression is valid, discarded or not.
#include <compare>

constexpr int discarded_statement(int a, int b) {
  (void)(a <=> b);
  (void)(1.5 <=> 2.5);
  return 1;
}
static_assert(discarded_statement(1, 2) == 1);

constexpr int discarded_comma(int a, int b) { return ((void)(a <=> b), a + b); }
static_assert(discarded_comma(3, 4) == 7);

constexpr int pointers() {
  int arr[2] = {};
  (void)(&arr[0] <=> &arr[1]);
  return (&arr[0] <=> &arr[1]) < 0;
}
static_assert(pointers() == 1);

constexpr bool used(int a, int b) { return (a <=> b) == std::strong_ordering::less; }
static_assert(used(1, 2));
static_assert(!used(2, 1));
