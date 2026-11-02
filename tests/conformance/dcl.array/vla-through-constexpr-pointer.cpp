// [dcl.array]/1, [expr.const]/4: a function-scope array bound read through
// a constexpr pointer to a variable that is not usable in constant
// expressions stays a variable length array (clang's extension), with the
// note naming the read; the lax namespace-scope fold
// (expr.const/lax-fold-through-pointer.cpp) does not apply here.
//@ args: -Wvla-cxx-extension
//@ strict-diags
//@ diag: 19:9 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 19:9 note "read of non-const variable 'n' is not allowed in a constant expression"
//@ diag: 14:5 note "declared here"
//@ diag: 20:9 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 20:14 note "read of non-constexpr variable 'd' is not allowed in a constant expression"
//@ diag: 15:14 note "declared here"
int n = 3;
const double d = 2.0;
constexpr int *p1 = &n;
constexpr const double *pd = &d;
void f() {
  int a[*p1];
  int b[(int)*pd];
  (void)a;
  (void)b;
}
