// [expr.const]/5.20, [expr.delete]/10: a delete-expression that selects a
// class-specific destroying `operator delete` is not a core constant
// expression — the deallocation function is not a replaceable global
// one — both for a direct delete and through a virtual destructor
// (clang `HandleOperatorDeleteCall`: "call to class-specific 'operator
// delete'").
//@ expect: compile-fail
//@ strict-diags
//@ diag: 24:15 error "static assertion expression is not an integral constant expression"
//@ diag: 23:38 note "call to class-specific 'operator delete'"
//@ diag: 24:15 note "in call to 'f()'"
//@ diag: 31:15 error "static assertion expression is not an integral constant expression"
//@ diag: 30:35 note "call to class-specific 'operator delete'"
//@ diag: 31:15 note "in call to 'g()'"
namespace std {
struct destroying_delete_t { explicit destroying_delete_t() = default; };
inline constexpr destroying_delete_t destroying_delete{};
}
struct S {
  int v;
  constexpr void operator delete(S *p, std::destroying_delete_t) { p->~S(); ::operator delete(p); }
};
constexpr int f() { S *p = new S{1}; delete p; return 1; }
static_assert(f() == 1);
struct V {
  virtual constexpr ~V() {}
  constexpr void operator delete(V *p, std::destroying_delete_t) { p->~V(); ::operator delete(p); }
};
struct W : V {};
constexpr int g() { V *p = new W; delete p; return 1; }
static_assert(g() == 1);
