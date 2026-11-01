// Inheriting constructors of variadic base constructors (WP-146): a
// C-variadic base constructor cannot have its arguments forwarded, so the
// inheriting constructor is inlined at each call site (clang
// `EmitInlinedInheritingCXXConstructorCall`), including when it is itself
// inherited again; a constructor template specialization is mangled with
// its template arguments and the pattern's parameter types, and the base
// subobject is constructed by its C2 variant.
extern "C" int printf(const char*, ...);
struct B { B(int n, ...) : s(n) { printf("B %d\n", n); } int s; };
struct M { M() : m(7) {} int m; };
struct D : B { using B::B; int x = 5; M mm; };
struct E : D { using D::D; int y = 9; };
template <class... T> struct TB { TB(T... t) : n(sizeof...(T)) {} int n; };
struct TD : TB<int, int> { using TB::TB; };
struct VB { template <class... T> VB(T... t) : k(sizeof...(T)) {} int k; };
struct VD : VB { using VB::VB; };
int main() {
  D d(3, 1, 2.5, 3);
  E e(2, 10, 20);
  D* p = new D(4, 1, 1, 1, 1);
  VD v(1, 2, 3, 4);
  TD t(1, 2);
  printf("%d %d %d %d %d %d %d %d\n", d.s, d.x, d.mm.m, e.s, e.y, p->x, v.k, t.n);
  delete p;
  return 0;
}
