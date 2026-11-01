//@ expect: run
//@ stdout: 4 3 1 5 2 3 3 2 1 4
// [basic.scope.class]/1, [basic.lookup.unqual]/1, [dcl.meaning]: after a
// qualified declarator-id `N::name`, the rest of the declarator (its
// parameters, array bounds, trailing return type and trailing
// requires-clause) and its initializer are looked up in the scope of `N`.
// A class member found there in a trailing return type may be named
// through `this`. WP-121. Members of a class template:
// `qualified-declarator-scope-template.cpp`.
extern "C" int printf(const char *, ...);
namespace N {
struct A { int a; };
A f(A);
extern const int K;
extern int arr[3];
} // namespace N
N::A N::f(A x) { return x; }
const int N::K = sizeof(A);
int N::arr[3] = {sizeof(A)};
struct S {
  using T = int;
  static const int N = 3;
  int v;
  auto f() -> decltype(v);
  T g(T);
  int h(int (&a)[N]);
  template <class U> auto k(U u) -> decltype(u + v);
  struct In { int q; };
  In m(In);
  auto self() const -> decltype(this);
  static const int M;
  static int w[2];
};
auto S::f() -> decltype(v) { return v; }
S::T S::g(T x) { return x; }
int S::h(int (&a)[N]) { return a[0]; }
template <class U> auto S::k(U u) -> decltype(u + v) { return u + v; }
S::In S::m(In i) { return i; }
auto S::self() const -> decltype(this) { return this; }
const int S::M = sizeof(T) / 2;
int S::w[2] = {M, N};
int main() {
  S s{4};
  int a[3]{1};
  printf("%d %d %d %d %d %d %d %d %d %d\n", s.f(), s.g(3), s.h(a), s.k(1),
         s.m({2}).q, S::w[1], N::f({3}).a, S::M, s.self() == &s, N::K);
}
