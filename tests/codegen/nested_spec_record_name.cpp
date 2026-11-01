// [WP-156] A class nested in a class template specialization is named
// with the enclosing specialization's arguments (clang
// printQualifiedName: `%"struct.N::A<long, 3>::In"`), and `T()` passes
// the default constructor's default arguments.
namespace N {
template <class T, int K> struct A {
  struct In { int x; };
  template <class U> struct M { U u; };
};
}
struct C { C(int a = 3) : z(a) {} int z; };
int g() {
  N::A<long, 3>::In i{1};
  N::A<int, 2>::M<char> m{};
  return i.x + m.u + C().z;
}
