// [class.temporary]/6, CWG1815 (WP-874): a temporary created by a default
// member initializer used in a variable's aggregate initialization is not
// lifetime-extended by clang 19, which warns that the extension is
// unsupported; each member using one is diagnosed at the closing brace (the
// array filler once), followed by a note at the member. clang 20+ extends
// it silently (the sibling dangling-default-member-init.cpp).
//@ strict-diags
//@ requires: clang<22
//@ expect: compile-ok
//@ diag: 42:7 warning "lifetime extension of temporary created by aggregate initialization using a default member initializer is not yet supported; lifetime of temporary will end at the end of the full-expression"
//@ diag: 36:23 note "initializing field 'r' with default member initializer"
//@ diag: 43:8 warning "lifetime extension of temporary created by aggregate initialization using a default member initializer is not yet supported; lifetime of temporary will end at the end of the full-expression"
//@ diag: 37:30 note "initializing field 'r' with default member initializer"
//@ diag: 44:11 warning "lifetime extension of temporary created by aggregate initialization using a default member initializer is not yet supported; lifetime of temporary will end at the end of the full-expression"
//@ diag: 36:23 note "initializing field 'r' with default member initializer"
//@ diag: 45:7 warning "lifetime extension of backing array of initializer list created by aggregate initialization using a default member initializer is not yet supported; lifetime of backing array will end at the end of the full-expression"
//@ diag: 38:39 note "initializing field 'l' with default member initializer"
//@ diag: 46:7 warning "lifetime extension of temporary created by aggregate initialization using a default member initializer is not yet supported; lifetime of temporary will end at the end of the full-expression"
//@ diag: 36:23 note "initializing field 'r' with default member initializer"
//@ diag: 46:7 warning "lifetime extension of backing array of initializer list created by aggregate initialization using a default member initializer is not yet supported; lifetime of backing array will end at the end of the full-expression"
//@ diag: 38:39 note "initializing field 'l' with default member initializer"
//@ diag: 47:15 warning "lifetime extension of temporary created by aggregate initialization using a default member initializer is not yet supported; lifetime of temporary will end at the end of the full-expression"
//@ diag: 36:23 note "initializing field 'r' with default member initializer"
//@ diag: 48:19 warning "lifetime extension of temporary created by aggregate initialization using a default member initializer is not yet supported; lifetime of temporary will end at the end of the full-expression"
//@ diag: 36:23 note "initializing field 'r' with default member initializer"

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
