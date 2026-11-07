// [namespace.memdef]/3, [basic.lookup.argdep]/4.2, [dcl.meaning.general]:
// a friend that redeclares a hidden friend stays hidden, and argument-
// dependent lookup still finds the function through any of its friend
// declarations in an associated class, also when a later friend
// declaration in an unrelated class is the latest redeclaration (a
// template's latest declaration holds the lookup entry).
//@ std: c++20
namespace N {
struct A {
  template <class T> friend constexpr int g(T, A) { return 1; }
  friend constexpr int h(A) { return 2; }
};
struct B {
  template <class T> friend constexpr int g(T, A);
  friend constexpr int h(A);
};
struct C;
struct D {
  template <class T> friend constexpr int k(T, C);
};
struct C {
  template <class T> friend constexpr int k(T, C) { return 3; }
};
} // namespace N

static_assert(g(1, N::A{}) == 1);
static_assert(h(N::A{}) == 2);
static_assert(k(1, N::C{}) == 3);

int main() { return g(1, N::A{}) + h(N::A{}) + k(1, N::C{}) == 6 ? 0 : 1; }
