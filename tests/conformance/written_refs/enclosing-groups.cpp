// The written-reference table (spec/ast.md §18.3.1): a decl-specifier-seq
// is shared by every declarator of its group ([dcl.decl.general]/3), so
// its names are held by the group: `in=` the first declaration, `shared=`
// the others. Variables at namespace and block scope, a member
// declarator list ([class.mem.general]), a typedef list, a function and
// a variable in one group. A class the specifier defines keeps its own
// names; a declarator's own names are its own; a structured binding
// declaration ([dcl.struct.bind]) and a group of one share nothing.
//@ written-refs: enclosing-groups.refs
namespace lib {
struct Shape { int w; };
template <class T> struct Box {};
} // namespace lib

lib::Shape first, second;
const lib::Box<lib::Shape> *ptr, value;
typedef lib::Shape T1, T2;
lib::Shape make(), made;
lib::Shape alone;
struct Pair { lib::Shape a; int b; } p1, p2;

struct Members {
  lib::Shape left, right;
  lib::Box<int> one;
};

int local() {
  lib::Shape x, y = x;
  auto [a, b] = Pair{};
  (void)a;
  (void)b;
  return y.w;
}
