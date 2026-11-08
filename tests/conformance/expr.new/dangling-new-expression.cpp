// [expr.new]/24, [class.temporary]/6.10 (WP-874): a temporary bound to a
// reference member of an object created by a new-expression, or an
// initializer list's backing array (directly allocated or a subobject),
// persists only until the end of the full-expression; clang warns (also
// through default member initializers). Binding a non-temporary is fine.
//@ strict-diags
//@ expect: compile-ok
//@ diag: 33:16 warning "temporary bound to reference member of allocated object will be destroyed at the end of the full-expression"
//@ diag: 34:17 warning "temporary bound to reference member of allocated object will be destroyed at the end of the full-expression"
//@ diag: 35:43 warning "array backing the allocated initializer list will be destroyed at the end of the full-expression"
//@ diag: 36:17 warning "temporary bound to reference member of allocated object will be destroyed at the end of the full-expression"
//@ diag: 26:23 note "initializing field 'r' with default member initializer"
//@ diag: 37:17 warning "array backing initializer list subobject of the allocated object will be destroyed at the end of the full-expression"
//@ diag: 27:39 note "initializing field 'l' with default member initializer"
//@ diag: 38:17 warning "array backing initializer list subobject of the allocated object will be destroyed at the end of the full-expression"

namespace std {
template <class E> class initializer_list {
  const E *b;
  decltype(sizeof 0) n;
public:
  constexpr initializer_list() : b(nullptr), n(0) {}
};
} // namespace std

struct A { const int &r = 0; };
struct C { std::initializer_list<int> l = {1, 2}; };
struct D { const int &r; };
struct E { D d; };
struct F { std::initializer_list<int> l; };
int g;
void f() {
  D *p = new D{0};
  E *q = new E{{1}};
  auto *l = new std::initializer_list<int>{1, 2};
  A *pa = new A{};
  C *pc = new C{};
  F *pf = new F{{3, 4}};
  D *ok = new D{g};
  int *pi = new int(g);
  (void)p; (void)q; (void)l; (void)pa; (void)pc; (void)pf; (void)ok; (void)pi;
}
