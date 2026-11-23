// Lookup provenance (spec/ast.md §18.7.3, [namespace.udir],
// [namespace.qual]/2): the using-directive a lookup went through, shown
// as `udir=<nominated namespace>@<directive>`. A directive at file
// scope, in a block and in a namespace; a transitive chain (the
// innermost link, the directive nominating the declaration's
// namespace); an inline namespace (not a directive: its members are
// found as members of the enclosing namespace); a directive written in
// an inline namespace (it counts as the enclosing namespace's); a
// using-declaration (`found` is the using-shadow and there is no
// directive) and one reached through a directive (both); a qualified
// lookup that searches the qualifier's nominated namespaces; a
// qualified name through a namespace alias (no directive); a name the
// scope declares itself (no directive though one is visible).
//@ written-refs: provenance-udir.refs
namespace a {
int x;
int y;
namespace inner {
int deep;
}
inline namespace v1 {
int z;
}
} // namespace a
namespace b {
using namespace a;
int w;
} // namespace b
namespace c {
using namespace b;
} // namespace c
namespace d {
int dv;
}
namespace al = a;

using namespace d;

int file_scope() { return dv; }

int in_block() {
  using namespace a;
  return x + z;
}

namespace c {
int in_namespace() { return w + y; }
} // namespace c

int qualified() { return b::x + al::y + a::z + a::inner::deep + ::dv; }

namespace e {
using a::y;
int f() { return y; }
} // namespace e
namespace g {
using namespace e;
int h() { return y; }
} // namespace g

namespace dd {
int dk;
}
namespace k {
inline namespace v2 {
using namespace dd;
}
int m() { return dk; }
} // namespace k

namespace own {
int dv;
int n() { return dv; }
} // namespace own
