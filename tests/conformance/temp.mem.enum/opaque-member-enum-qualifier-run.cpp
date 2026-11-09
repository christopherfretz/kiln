// [temp.mem.enum]/1, [temp.dep.type]/6, [temp.res.general]: an opaque member
// enumeration of a class template, defined outside the class after the member
// functions that name its enumerators through it (`E::a`), is a dependent type
// not yet defined there: the qualified name is looked up at instantiation, where
// the definition is known (clang `computeDeclContext`; found by WP-600).
//@ expect: run
//@ strict-diags
//@ stdout: 5 7 9
#include <cstdio>
template <class T>
struct A {
  enum class E : int;
  enum U : int;
  int g() { return (int)E::a; }
  int h();
  static int k() { return U::u + 1; }
};
template <class T>
enum class A<T>::E : int { a = 5 };
template <class T>
enum A<T>::U : int { u = 8 };
template <class T>
int A<T>::h() { return (int)E::a + 2; }
int main() { std::printf("%d %d %d\n", A<int>().g(), A<int>().h(), A<int>::k()); }
