// [expr.new]/8: an array bound that is negative, too large, over the
// implementation limit or smaller than the number of initializer-clauses makes
// the new-expression throw, which is not a constant expression
// ([expr.const]/5); the non-throwing form yields a null pointer instead.
// Mirrors crates/kiln_oven/tests/new_delete.rs `expr_new_bound_negative`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 31:15 error "static assertion expression is not an integral constant expression"
//@ diag: 27:20 note "cannot allocate array; evaluated array bound -1 is negative"
//@ diag: 31:15 note "in call to 'make(-1)'"
//@ diag: 32:15 error "static assertion expression is not an integral constant expression"
//@ diag: 27:20 note "cannot allocate array; evaluated array bound 1073741824 exceeds the limit (1048576); use '-fconstexpr-steps' to increase this limit"
//@ diag: 32:15 note "in call to 'make(1073741824)'"
//@ diag: 33:15 error "static assertion expression is not an integral constant expression"
//@ diag: 27:20 note "cannot allocate array; evaluated array bound 2000000 exceeds the limit (1048576); use '-fconstexpr-steps' to increase this limit"
//@ diag: 33:15 note "in call to 'make(2000000)'"
//@ diag: 40:15 error "static assertion expression is not an integral constant expression"
//@ diag: 36:20 note "cannot allocate array; evaluated array bound 2 is too small to hold 3 explicitly initialized elements"
//@ diag: 40:15 note "in call to 'list(2)'"
namespace std {
struct nothrow_t {};
extern const nothrow_t nothrow;
} // namespace std
void *operator new[](unsigned long, const std::nothrow_t &) noexcept;

constexpr int make(int n) {
  int *p = new int[n];
  delete[] p;
  return 1;
}
static_assert(make(-1));
static_assert(make(1 << 30));
static_assert(make(2000000));

constexpr int list(int n) {
  int *p = new int[n]{1, 2, 3};
  delete[] p;
  return 1;
}
static_assert(list(2));

// The non-throwing form yields a null pointer instead.
constexpr bool null(int n) {
  int *p = new (std::nothrow) int[n]{1, 2, 3};
  bool r = p == nullptr;
  delete[] p;
  return r;
}
static_assert(null(-1) && null(1 << 30) && null(2) && !null(3));
