// [temp.inst]/2-3, [basic.def.odr]: a function whose parameter and
// return types are a class template specialization, a member class of
// one, or an explicit specialization completed only by a later top-level
// declaration is referenced (address taken) before that point, then
// called through the pointer and directly once the class is complete
// (WP-492: kiln declares it `void ()` there as clang does; the call
// sites use their own function type).
//@ expect: run
//@ stdout: 42 17 20 | 8 6
extern "C" int printf(const char*, ...);
template <class T> struct X { T t; };
template <class T> struct O { struct I { T v; }; };
template <class T> struct S;
X<int> g(X<int>);
O<int>::I gi(O<int>::I);
S<int> gs(S<int>);
X<int> (*pg)(X<int>);
O<int>::I (*pgi)(O<int>::I);
S<int> (*pgs)(S<int>);
void h() { pg = &g; pgi = &gi; pgs = &gs; }
template <> struct S<int> { long a, b; };
namespace N {
struct A;
int ga(A);
int (*pa)(A);
void ha() { pa = &ga; }
struct A { int x; };
}
int main() {
  h();
  N::ha();
  X<int> x = pg(X<int>{21});
  O<int>::I i = gi(O<int>::I{4});
  S<int> s = pgs(S<int>{9, 11});
  printf("%d %d %ld | %d %d\n", x.t, pgi(i).v + 1, s.a + s.b, N::pa(N::A{4}),
         N::ga(N::A{3}));
}
X<int> g(X<int> x) { return {x.t * 2}; }
O<int>::I gi(O<int>::I i) { return {i.v * 2}; }
S<int> gs(S<int> s) { return {s.b, s.a}; }
int N::ga(A a) { return a.x * 2; }
