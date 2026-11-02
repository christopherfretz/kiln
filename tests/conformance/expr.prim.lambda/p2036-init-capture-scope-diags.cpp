// [expr.prim.lambda.capture]/6 (P2036R3, a DR so also C++20): the
// init-capture found in the noexcept-specifier is the lambda's variable,
// not a constant; a name that is neither a capture nor declared is still
// undeclared in the trailing return type.
//@ expect: compile-fail
//@ std: c++20
//@ strict-diags
//@ diag: 13:31 error "noexcept specifier argument is not a constant expression"
//@ diag: 13:31 note "read of non-const variable 'y' is not allowed in a constant expression"
//@ diag: 13:13 note "declared here"
//@ diag: 14:34 error "use of undeclared identifier 'w'"
void f() {
  auto b = [y = 1]() noexcept(y) { return 0; };
  auto d = [y = 1]() -> decltype(w) { return y; };
}
