// [expr.const]/5.9, [basic.def]/2: the "declared here" note on a read of a
// variable whose value is not usable names its definition once seen (clang
// `findCompleteObject`), a usable variable's notes its first declaration;
// an array bound reading a const variable with a non-constant initializer
// says "initializer of 'k' is not a constant expression".
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 44:15 error "constexpr variable 'r1' must be initialized by a constant expression"
//@ diag: 44:20 note "read of non-const variable 'a' is not allowed in a constant expression"
//@ diag: 43:5 note "declared here"
//@ diag: 46:15 error "constexpr variable 'r2' must be initialized by a constant expression"
//@ diag: 46:20 note "read of non-const variable 'a' is not allowed in a constant expression"
//@ diag: 43:5 note "declared here"
//@ diag: 49:15 error "constexpr variable 'r3' must be initialized by a constant expression"
//@ diag: 49:20 note "read of non-const variable 'g' is not allowed in a constant expression"
//@ diag: 47:5 note "declared here"
//@ diag: 51:15 error "constexpr variable 'r4' must be initialized by a constant expression"
//@ diag: 51:20 note "read of non-const variable 'h' is not allowed in a constant expression"
//@ diag: 50:26 note "declared here"
//@ diag: 53:15 error "constexpr variable 'r5' must be initialized by a constant expression"
//@ diag: 53:20 note "read of non-const variable 'h' is not allowed in a constant expression"
//@ diag: 52:8 note "declared here"
//@ diag: 56:18 error "constexpr variable 'r6' must be initialized by a constant expression"
//@ diag: 56:23 note "read of non-constexpr variable 'd' is not allowed in a constant expression"
//@ diag: 55:14 note "declared here"
//@ diag: 63:15 error "constexpr variable 'r7' must be initialized by a constant expression"
//@ diag: 63:20 note "initializer of 'c' is not a constant expression"
//@ diag: 60:18 note "declared here"
//@ diag: 66:15 error "constexpr variable 'r8' must be initialized by a constant expression"
//@ diag: 66:20 note "initializer of 'e' is unknown"
//@ diag: 64:18 note "declared here"
//@ diag: 69:15 error "constexpr variable 'r9' must be initialized by a constant expression"
//@ diag: 69:20 note "initializer of 's' is not a constant expression"
//@ diag: 67:29 note "declared here"
//@ diag: 73:9 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 73:9 note "initializer of 'k' is not a constant expression"
//@ diag: 72:11 note "declared here"
//@ diag: 73:5 error "variable length array declaration not allowed at file scope"
// A variable that is not usable in constant expressions: the note names
// its definition, else the declaration the read names.
extern int a;
int a = 3;
constexpr int r1 = a;
extern int a;
constexpr int r2 = a + 1;
int g;
extern int g;
constexpr int r3 = (g);
namespace n { extern int h; }
constexpr int r4 = n::h;
int n::h = 2;
constexpr int r5 = n::h;
extern const double d;
const double d = 2.5;
constexpr double r6 = d;

// A usable variable: the first declaration.
int m = 1;
extern const int c;
const int c = m;
extern const int c;
constexpr int r7 = c;
extern const int e;
extern const int e;
constexpr int r8 = e;
struct S { static const int s; };
const int S::s = m;
constexpr int r9 = S::s;

// An array bound reading one at namespace scope.
const int k = m;
int arr[k];
