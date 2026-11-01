// [support.srcloc.cons]/2, [dcl.init.aggr]/5 (WP-158 item f, covered by
// WP-153): the location builtins of a default member initializer used by
// an aggregate initialization take the initialization's location, also
// in a constant expression, inside a constexpr function and with
// designated initializers.
//@ strict-diags
#include <source_location>
struct S {
  int line = __builtin_LINE();
  const char *fn = __builtin_FUNCTION();
  unsigned col = __builtin_COLUMN();
  std::source_location loc = std::source_location::current();
};
constexpr S a{};
static_assert(a.line == 14 && a.col == 15 && a.loc.line() == 14);
constexpr int g() { S s{}; return s.line; }
static_assert(g() == 16);
constexpr bool fn_is_g() { S s{}; return s.fn[0] == 'f' && s.fn[1] == 'n' && s.fn[2] == '_'; }
static_assert(fn_is_g());
constexpr S b{.line = 5};
static_assert(b.line == 5 && b.col == 24 && b.loc.line() == 20);
