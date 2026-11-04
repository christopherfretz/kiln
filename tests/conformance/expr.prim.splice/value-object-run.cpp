// [expr.prim.splice]/2 (P2996R13) through codegen: value splices emit
// their values (an int, an enumerator, a double, a char, a pointer to
// function called through, pointers to member applied, a pointer to an
// array element dereferenced) and object splices emit the object's
// address, so a store through the splice of a mutable array element is
// seen by the program and a member of a template parameter object reads
// its value. A reflection of a virtual base subobject of a variable, and
// of one inside a member, names that subobject. GCC 16.1 compiles and
// runs the same file with the same output.
//@ expect: run
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
//@ stdout: 5 11 2.5 c 9 42 3 6 20
//@ stdout: 33 1 y 1
//@ stdout: 4 4 8 1
#include <meta>
#include <cstdio>

enum class Shade { dim = 11 };
int add_four(int a) { return a + 4; }
struct Pair { int first; int second() const { return 3; } };
int slots[3] = {10, 20, 30};
struct Tag { char s[4]{}; };
constexpr Pair fixed{42};
struct Root { int v; };
struct Mid : virtual Root { int m; };
struct Holder { int pad; Mid inner; };
Mid mid;
Holder holder;

constexpr auto rint = std::meta::reflect_constant(5);
constexpr auto rshade = std::meta::reflect_constant(Shade::dim);
constexpr auto rdouble = std::meta::reflect_constant(2.5);
constexpr auto rchar = std::meta::reflect_constant('c');
constexpr auto rfn = std::meta::reflect_constant(&add_four);
constexpr auto rmem = std::meta::reflect_constant(&Pair::first);
constexpr auto rmfn = std::meta::reflect_constant(&Pair::second);
constexpr auto rptr = std::meta::reflect_constant(&slots[1]);
constexpr auto rslot = std::meta::reflect_object(slots[0]);
constexpr auto rtag = std::meta::reflect_constant(Tag{{'x', 'y'}});
constexpr auto rfixed = std::meta::reflect_object(fixed);
constexpr auto rroot = std::meta::reflect_object(static_cast<Root&>(mid));
constexpr auto rinner = std::meta::reflect_object(static_cast<Root&>(holder.inner));

int main() {
  Pair p{6};
  std::printf("%d %d %.1f %c %d %d %d %d %d\n", [:rint:], static_cast<int>([:rshade:]),
              [:rdouble:], [:rchar:], [:rfn:](5), [:rfixed:].*[:rmem:], (p.*[:rmfn:])(),
              p.*[:rmem:], *[:rptr:]);
  [:rslot:] = 33;
  int* where = &[:rslot:];
  std::printf("%d %d %c %d\n", slots[0], where == &slots[0], [:rtag:].s[1],
              &[:rtag:] == &[:std::meta::reflect_constant(Tag{{'x', 'y'}}):]);
  mid.v = 3;
  holder.inner.v = 8;
  [:rroot:].v += 1;
  std::printf("%d %d %d %d\n", [:rroot:].v, mid.v, [:rinner:].v,
              &[:rinner:] == static_cast<Root*>(&holder.inner));
}
