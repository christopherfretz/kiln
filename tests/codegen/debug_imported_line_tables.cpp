// Debug info (WP-1365): with line tables only (`-gline-tables-only`, also
// `-g1`) the compile unit has `emissionKind: LineTablesOnly` and no
// `globals:`, `retainedTypes:` or `imports:`, and no variable is described
// (`#dbg_declare`): not the globals, locals, statics in functions,
// structured bindings, nor the using-directives, using-declarations and
// namespace aliases ([namespace.udir], [namespace.udecl],
// [namespace.alias]). llc 22 rejects a line-tables-only unit that carries
// them. kiln_driver's `line_tables_only_has_no_variables` checks the same
// unit for their absence at `-gline-tables-only`, `-g1` and `-O1`.
//@ args: -gline-tables-only
namespace lib {
int counter = 1;
extern int declared;
struct Pair {
  int a, b;
};
enum Color { red, green };
int twice(int x) { return 2 * x; }
} // namespace lib
namespace alias = lib;
using namespace lib;
using lib::counter;
using lib::declared;
using lib::twice;
using lib::Pair;
static int file_static = 3;
int user(int n) {
  using namespace alias;
  using lib::Color;
  namespace inner = lib;
  static int calls = 0;
  ++calls;
  Pair p{n, 2};
  auto [x, y] = p;
  Color c = red;
  int local = x + y + c + inner::counter;
  return local + twice(calls) + file_static;
}
