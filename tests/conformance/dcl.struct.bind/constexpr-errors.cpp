//@ expect: compile-fail
//@ strict-diags
//@ diag: 16:15 error "static assertion expression is not an integral constant expression"
//@ diag: 16:15 note "read of non-constexpr variable '[a, b]' is not allowed in a constant expression"
//@ diag: 15:12 note "declared here"
//@ diag: 22:15 error "static assertion expression is not an integral constant expression"
//@ diag: 20:10 note "read of dereferenced one-past-the-end pointer is not allowed in a constant expression"
//@ diag: 22:15 note "in call to 'f()'"
// [expr.const]/5: the unnamed holding variable of a structured binding
// declaration is not usable in constant expressions unless it is a
// constexpr object's reference or of const integral type; clang names it
// `[a, b]` in the note. Undefined behavior through a binding (here
// past-the-end access) is diagnosed as for any lvalue.
struct P { int x, y; };
const auto [a, b] = P{1, 2};
static_assert(a == 1);
constexpr int f() {
  int arr[2] = {1, 2};
  auto &[u, v] = arr;
  return (&u)[2];
}
static_assert(f() == 0);
