// [dcl.contract.func]/1-4, [dcl.decl.general]/1 (P2900R14): a
// function-contract-specifier-seq follows the declarator, after a
// trailing requires-clause and, for a member, after its virt-specifiers.
// It may appear on free functions, members (a complete-class context, so a
// later member is usable), static members, constructors, destructors,
// friend definitions, templates, class template members, explicit
// specializations (a first declaration of their own), lambdas and a
// function with a deduced return type defined with it. A redeclaration
// repeats the sequence or omits it. GCC 16.1 agrees.
//@ expect: compile-ok
//@ strict-diags
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ oracle: gcc
int f(const int x) pre(x > 0) post(r: r > x);
int f(const int x) pre(x > 0) post(r: r > x) { return x + 1; }
int f(const int x);
int g(int x) pre(x > 0);
int g(int x) { return x; }
struct S {
  int m;
  int get(const int a) const pre(a > m) post(r: r == a) { return a; }
  int later() pre(k > 0) { return k; }
  int k;
  static int st(int a) pre(a != 0);
  friend int fr(int a) pre(a > 0) { return a; }
  S(int v) pre(v >= 0) : m(v), k(v) {}
  ~S() post(true) {}
};
int S::st(int a) { return a; }
template <class T> T tf(const T t) pre(t > T{}) post(r: r >= t) { return t; }
template <class T> struct C {
  T m;
  T get(const T a) const pre(a > m) post(r: r == a) { return a; }
};
template <class T> T sp(T t) pre(t > 0);
template <> int sp<int>(int t) pre(t > 1) { return t; }
auto lam = [](int a) pre(a > 0) post(r: r > 0) { return a; };
auto ded() post(r: r > 0) { return 1; }
template <class T> void rq(T t) requires (sizeof(T) > 1) pre(t != 0) {}
int use() {
  rq(2L);
  return f(1) + g(1) + tf(2) + C<int>{0}.get(1) + sp(2) + lam(1) + ded() + S::st(1);
}
