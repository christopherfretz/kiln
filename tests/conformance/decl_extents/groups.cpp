// The declaration-extent table (spec/ast.md §19.5): a declarator group
// shares its full range and decl-specifier prefix (`specs-end=`); each
// declarator's own range is its init-declarator. A class defined in the
// group's specifiers has its own row first; structured bindings, member
// declarator lists, bit-fields and using-declarator lists.
//@ decl-extents: groups.extents
//@ std: c++20
int a, *b = 0;
static const int c[2] = {1, 2}, &d = c[0];
struct P {
  int x, y : 4, z = 3;
} p, *pp;
struct Two {
  int l, r;
};
auto [u, v] = Two{};
namespace m {
int f();
int g();
} // namespace m
using m::f, m::g;
typedef int I1, *I2;
void fn(int first, int = 2, int (*cb)(int) = nullptr);
