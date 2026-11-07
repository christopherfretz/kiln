// [expr.new]/8, [conv]/5 (contextual implicit conversion), [class.conv.fct]/9:
// a class-type array bound converts through a conversion function the
// class inherits, one a using-declaration names from a private base
// ([namespace.udecl]/1, /19), or a base's conversion function template.
//@ expect: run
//@ stdout: 2 3 4
#include <cstdio>

struct B { operator int() const { return 2; } };
struct D : B {};
template <class T> struct TB { template <class U = T> operator U() const { return 4; } };
struct TD : TB<int> {};
struct P3 : private B { using B::operator int; };

int main() {
  int* a = new int[D()];
  int* b = new int[static_cast<int>(P3()) + 1];
  int* c = new int[TD()];
  a[1] = 2; b[2] = 3; c[3] = 4;
  std::printf("%d %d %d\n", a[1], b[2], c[3]);
  delete[] a; delete[] b; delete[] c;
}
