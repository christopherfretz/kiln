// [expr.const]/5.18 (C++26, P2747R2): the reserved placement 'operator new'
// may be evaluated (clang 22's VisitCXXNewExpr); any other placement form,
// here an alignment argument, is not supported in a constant expression.
//@ expect: compile-fail
//@ requires: clang>=22
//@ strict-diags
//@ diag: 26:15 error "static assertion expression is not an integral constant expression"
//@ diag: 23:12 note "this placement new expression is not supported in constant expressions "
//@ diag: 26:15 note "in call to 'aligned()'"
void *operator new(unsigned long, void *p) noexcept { return p; }
namespace std {
enum class align_val_t : unsigned long {};
}

constexpr int place() {
  int x = 0;
  int *p = new (&x) int(1);
  return *p;
}
static_assert(place() == 1);

constexpr int aligned() {
  int *p = new (std::align_val_t(16)) int;
  return 1;
}
static_assert(aligned());
