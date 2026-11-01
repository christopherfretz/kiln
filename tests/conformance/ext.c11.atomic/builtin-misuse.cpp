// Clang extension: misuse of the atomic builtins is diagnosed as clang's
// BuildAtomicExpr does: pointer, _Atomic, const, integer/pointer/fp and
// trivially-copyable pointee requirements, completeness and arity.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 21:3 error "address argument to atomic operation must be a pointer to _Atomic type ('int *' invalid)"
//@ diag: 22:3 error "address argument to atomic operation must be a pointer to non-const _Atomic type ('const _Atomic(int) *' invalid)"
//@ diag: 23:3 error "address argument to atomic operation must be a pointer to non-const type ('const int *' invalid)"
//@ diag: 24:3 error "address argument to atomic builtin must be a pointer ('int' invalid)"
//@ diag: 25:3 error "address argument to atomic operation must be a pointer to integer ('float *' invalid)"
//@ diag: 26:3 error "address argument to atomic operation must be a pointer to atomic integer ('_Atomic(float) *' invalid)"
//@ diag: 27:3 error "address argument to atomic operation must be a pointer to integer or pointer ('float *' invalid)"
//@ diag: 28:3 error "address argument to atomic operation must be a pointer to a trivially-copyable type ('NT *' invalid)"
//@ diag: 29:19 error "incomplete type 'Inc' where a complete type is required"
//@ diag: 19:8 note "forward declaration of 'Inc'"
//@ diag: 30:23 error "too few arguments to function call, expected 2, have 1"
//@ diag: 31:28 error "too many arguments to function call, expected 2, have 3"
struct NT { NT(const NT&); int x; };
struct Inc;
int f(int *ip, const int *cip, _Atomic(int) *ap, const _Atomic(int) *cap, float *fp, NT *nt, int i, Inc *inc, _Atomic(float) *af) {
  __c11_atomic_load(ip, 5);
  __c11_atomic_store(cap, 1, 5);
  __atomic_store_n(cip, 1, 5);
  __atomic_load_n(i, 5);
  __atomic_fetch_xor(fp, 1, 5);
  __c11_atomic_fetch_and(af, 1, 5);
  __atomic_exchange_n(fp, 1.0f, 5);
  __atomic_load(nt, nt, 5);
  __atomic_load_n(inc, 5);
  __c11_atomic_load(ap);
  __c11_atomic_load(ap, 5, 6);
  __atomic_fetch_max(fp, 1, 5); // floating min/max are allowed
  return 0;
}
