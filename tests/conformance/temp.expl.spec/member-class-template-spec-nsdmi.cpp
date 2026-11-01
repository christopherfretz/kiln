//@ expect: run
//@ oracle: none
//@ skip(clang): clang 19.1.7 hangs (never terminates) on a use of an explicitly specialized member class template whose specialization has a default member initializer; a dependent initializer (`U b = U();`) crashes it instead
// [temp.expl.spec]/17-18: a member class template of an implicitly
// instantiated class template specialization can be explicitly specialized
// as a template (`template<> template<class U> struct X<int>::M`). Its
// definition replaces the primary member's, default member initializers
// included ([class.mem.general]/11), for every M<U> of X<int>.
template<class T> struct X {
  template<class U> struct M { int a = 1; };
};
template<> template<class U> struct X<int>::M { int b = 2; U c = U(7); };

X<int>::M<char> m;

int main() {
  X<int>::M<long> n;
  X<short>::M<char> o;
  if (m.b != 2 || m.c != 7) return 1;
  if (n.b != 2 || n.c != 7) return 2;
  if (o.a != 1) return 3;
  return 0;
}
