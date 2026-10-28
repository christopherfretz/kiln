// [cpp.include]: an included header's declarations and macro definitions are
// visible after the #include, and its macros expand in the including file.
//@ expect: compile-ok
//@ strict-diags
#include "Inputs/decls.h"
DECLARE_COUNTER(local_counter, header_counter + 1)
static_assert(local_counter == 8);
static_assert(SQUARE(header_counter) == 49);
static_assert(header_square(5) == 25);
constexpr Point p{1, 2};
static_assert(p.x + p.y == 3);
static_assert(sizeof(Point) == 2 * sizeof(int));
