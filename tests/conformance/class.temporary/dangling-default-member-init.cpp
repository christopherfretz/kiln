// [class.temporary]/6, CWG1815 (WP-874): a temporary created by a default
// member initializer used in a variable's aggregate initialization is
// lifetime-extended with the variable (clang 20+, not diagnosed); clang 19
// warns instead (the sibling dangling-default-member-init-19.cpp).
//@ strict-diags
//@ requires: clang>=22
//@ expect: compile-ok

namespace std {
template <class E> class initializer_list {
  const E *b;
  decltype(sizeof 0) n;
public:
  constexpr initializer_list() : b(nullptr), n(0) {}
};
} // namespace std

struct A { const int &r = 0; };
struct B { int x; const int &r = 1; };
struct C { std::initializer_list<int> l = {1, 2}; };
struct D { A a; C c; };

void f() {
  A a{};
  B b{1};
  A a2 = {};
  C c{};
  D d{};
  A arr[2] = {};
  const A &ra = A{};
  A explicit_init{a.r}; // no default member initializer used
  (void)a; (void)b; (void)a2; (void)c; (void)d; (void)arr; (void)ra;
  (void)explicit_init;
}
