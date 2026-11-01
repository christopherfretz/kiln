// [basic.stc.auto]/1: a variable declared in the block of a member function, constructor or
// lambda has automatic storage duration; its address is not a constant (a static local's is)
//@ expect: run
//@ strict-diags
struct S {
  int g() const { int x = 3; int* p = &x; return *p; }
  int s() const { static int n = 0; static int* q = &n; return ++*q; }
  S() { int y = 4; int* p = &y; v = *p; }
  int v;
};
template <class C> struct T {
  int g() const { int x = 3; int* p = &x; return *p; }
  template <class I> int h(I i) const { I x = i; I* p = &x; return *p; }
  int s() const { static int n = 0; static int* q = &n; return ++*q; }
};
int main() {
  S a; T<long> t;
  auto l = [] { int z = 5; int* p = &z; return *p; };
  if (a.g() != 3 || a.v != 4 || a.s() != 1 || a.s() != 2) return 1;
  if (t.g() != 3 || t.h(7) != 7 || t.s() != 1 || t.s() != 2) return 2;
  return l() == 5 ? 0 : 3;
}
