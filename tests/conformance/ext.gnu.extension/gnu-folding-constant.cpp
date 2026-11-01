// [dcl.enum]/5, [class.bit]/1, [dcl.init.aggr] (C99 designator), GNU
// extension: an integral constant expression that is not one but folds
// (a const double read) is accepted with 'folding it to a constant is a
// GNU extension' [-Wgnu-folding-constant, off by default] for an
// enumerator without a fixed underlying type, a bit-field width and an
// array designator, and the folded value is used (clang
// VerifyIntegerConstantExpression AllowFold; WP-230)
//@ expect: compile-ok
//@ strict-diags
//@ args: -Wgnu-folding-constant
//@ diag: 32:15 warning "expression is not an integral constant expression; folding it to a constant is a GNU extension"
//@ diag: 32:20 note "read of non-constexpr variable 'd' is not allowed in a constant expression"
//@ diag: 31:14 note "declared here"
//@ diag: 32:28 warning "expression is not an integral constant expression; folding it to a constant is a GNU extension"
//@ diag: 32:33 note "read of non-constexpr variable 'd' is not allowed in a constant expression"
//@ diag: 31:14 note "declared here"
//@ diag: 34:20 warning "expression is not an integral constant expression; folding it to a constant is a GNU extension"
//@ diag: 34:25 note "read of non-constexpr variable 'd' is not allowed in a constant expression"
//@ diag: 31:14 note "declared here"
//@ diag: 34:57 warning "expression is not an integral constant expression; folding it to a constant is a GNU extension"
//@ diag: 34:62 note "read of non-constexpr variable 'd' is not allowed in a constant expression"
//@ diag: 31:14 note "declared here"
//@ diag: 36:26 warning "expression is not an integral constant expression; folding it to a constant is a GNU extension"
//@ diag: 36:31 note "read of non-constexpr variable 'd' is not allowed in a constant expression"
//@ diag: 31:14 note "declared here"
//@ diag: 36:25 warning "array designators are a C99 extension"
//@ diag: 38:8 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 38:13 note "read of non-constexpr variable 'd' is not allowed in a constant expression"
//@ diag: 31:14 note "declared here"
//@ diag: 38:5 warning "variable length array folded to constant array as an extension"
const double d = 2.0;
enum E { e1 = (int)d, e2 = (int)d + sizeof(int), e3 = (int)1.5 };
static_assert(e1 == 2 && e2 == 6 && e3 == 1);
struct S { int b : (int)d; int c : (int)3.0; unsigned : (int)d * 0; };
static_assert(sizeof(S) == 4);
constexpr int arr[3] = {[(int)d] = 7};
static_assert(arr[2] == 7 && arr[0] == 0);
int a1[(int)d];
static_assert(sizeof(a1) == 2 * sizeof(int));
