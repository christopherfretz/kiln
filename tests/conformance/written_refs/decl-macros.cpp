// The written-reference table (spec/ast.md §18.5) in declarative
// contexts (stage 3): a name from a macro body or argument keeps its
// token's location, so the dump shows its spelling and the outermost
// expansion (`exp=`): a using-declaration, a using-directive, a
// namespace alias target, a qualified definition, a friend, a
// specialization target, a type-constraint and a designator.
//@ written-refs: decl-macros.refs
namespace impl {
int compute(int);
struct Gear {
  int teeth;
};
template <class T> struct Tag {};
template <class T> concept Ok = true;
} // namespace impl
#define IMPL impl
#define USE(name) using IMPL::name;
#define DEFINE(name) int IMPL::name(int v) { return v; }
#define FIELD(f, v) .f = v

USE(compute)
using namespace IMPL;
namespace alias = IMPL;
DEFINE(compute)
class Friendly {
  friend struct IMPL::Gear;
};
template <> struct IMPL::Tag<int> {};
template <IMPL::Ok T> void constrained(T);
impl::Gear g = {FIELD(teeth, 12)};
