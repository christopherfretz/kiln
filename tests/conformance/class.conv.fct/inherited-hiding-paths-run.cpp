// [class.conv.fct]/9, [class.member.lookup]: conversion functions are
// inherited; a derived class's conversion function to the same type hides
// a base's along that path only. Through a virtual base reached along two
// paths, the base's is hidden when it is hidden along either (clang
// `CollectVisibleConversions`), whichever order the bases are declared
// in. A using-declaration names one of two sibling bases' functions, which
// then hides the other's ([namespace.udecl]/1). A template-derived base
// contributes its conversion function too.
//@ expect: run
//@ stdout: 2 2 4 7 8
#include <cstdio>

struct A { operator int() const { return 1; } };
struct B : virtual A { operator int() const { return 2; } };
struct C : virtual A {};
struct D : B, C {};
struct D2 : C, B {};

struct L { operator int() const { return 3; } };
struct R { operator int() const { return 4; } };
struct LRU : L, R { using R::operator int; };

template <class T> struct TB { operator T() const { return 7; } };
struct TD : TB<int> {};

struct Own : L { operator int() const { return 8; } };

int main() {
  int a = D();
  int b = D2();
  int c = LRU();
  int d = TD();
  int e = Own();
  std::printf("%d %d %d %d %d\n", a, b, c, d, e);
}
