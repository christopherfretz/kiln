// [namespace.udecl]/13, [over.best.ics.general]/7, [over.match.copy]/1.1:
// an inherited constructor is a converting constructor of the derived
// class, so it forms the user-defined conversion of an implicit
// conversion sequence: an argument of a function call, and the operand
// of assignment through the implicit copy and move assignment operators
// (abseil `set = initializer_list` through raw_hash_set's inherited
// initializer-list constructors).
//@ expect: run
//@ stdout: 3 102 7 9
#include <cstdio>
#include <initializer_list>
#include <type_traits>
struct B {
  B() {}
  template <class T, std::enable_if_t<std::is_same<T, long>::value, int> = 0>
  B(std::initializer_list<T> i, int = 0) : s(int(i.size()) + 100) {}
  B(std::initializer_list<int> i, int = 0) : s(int(i.size())) {}
  B(int v) : s(v) {}
  int s = 0;
};
struct D : B { using B::B; };
int take(const D &d) { return d.s; }
int sink(D &&d) { return d.s + 2; }
int main() {
  std::initializer_list<int> v = {1, 2, 3};
  D d;
  d = v;
  std::printf("%d ", d.s);
  std::initializer_list<long> w = {1, 2};
  d = w;
  std::printf("%d %d %d\n", d.s, take(7), sink(7));
}
