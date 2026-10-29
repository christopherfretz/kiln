// [expr.const]/5.20: only a replaceable global allocation function may be
// called in a constant evaluation. A placement 'operator new' is not one
// (clang 19 does not implement P2747), and clang 19 does not support an
// alignment argument either.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 23:15 error "static assertion expression is not an integral constant expression"
//@ diag: 20:12 note "call to placement 'operator new'"
//@ diag: 23:15 note "in call to 'place()'"
//@ diag: 29:15 error "static assertion expression is not an integral constant expression"
//@ diag: 26:12 note "this placement new expression is not yet supported in constant expressions"
//@ diag: 29:15 note "in call to 'aligned()'"
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
