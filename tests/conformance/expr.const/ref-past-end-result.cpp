// [expr.const]/22, [dcl.ref]/5: a reference constant expression refers to
// an object, never to one past the end of one (clang
// `CheckLValueConstantExpression`, both clang 19 and 22); a pointer may
// point there. "subobject of" when the reference designates a subobject.
// WP-1021.
//@ std: c++26
//@ expect: compile-fail
//@ strict-diags
//@ diag: 31:22 error "constexpr variable 'r1' must be initialized by a constant expression"
//@ diag: 31:22 note "dereferenced pointer past the end of 'x' is not a constant expression"
//@ diag: 30:15 note "declared here"
//@ diag: 34:22 error "constexpr variable 'r2' must be initialized by a constant expression"
//@ diag: 34:22 note "dereferenced pointer past the end of subobject of 'p' is not a constant expression"
//@ diag: 33:13 note "declared here"
//@ diag: 35:22 error "constexpr variable 'r3' must be initialized by a constant expression"
//@ diag: 35:22 note "dereferenced pointer past the end of subobject of 'p' is not a constant expression"
//@ diag: 33:13 note "declared here"
//@ diag: 36:20 error "constexpr variable 'r4' must be initialized by a constant expression"
//@ diag: 36:20 note "dereferenced pointer past the end of 'p' is not a constant expression"
//@ diag: 33:13 note "declared here"
//@ diag: 39:22 error "constexpr variable 'r5' must be initialized by a constant expression"
//@ diag: 39:22 note "dereferenced pointer past the end of subobject of 'arr' is not a constant expression"
//@ diag: 37:15 note "declared here"
//@ diag: 41:23 error "constexpr variable 'r6' must be initialized by a constant expression"
//@ diag: 41:23 note "dereferenced pointer past the end of subobject of 'arr' is not a constant expression"
//@ diag: 37:15 note "declared here"
//@ diag: 43:22 error "constexpr variable 'r8' must be initialized by a constant expression"
//@ diag: 43:22 note "dereferenced pointer past the end of subobject of temporary is not a constant expression"
//@ diag: 43:27 note "temporary created here"
constexpr int x = 1;
constexpr const int &r1 = *(&x + 1);
struct P { int a; int b; };
constexpr P p{1, 2};
constexpr const int &r2 = *(&p.b + 1);
constexpr const int &r3 = *(&p.a + 1);
constexpr const P &r4 = *(&p + 1);
constexpr int arr[2][2] = {};
constexpr const int *e5 = arr[0] + 2;
constexpr const int &r5 = *e5;
constexpr const int (*e6)[2] = arr + 2;
constexpr const int (&r6)[2] = *e6;
struct A { int v[2]; };
constexpr const int &r8 = A{{1, 2}}.v[2];
constexpr const int *ok1 = &x + 1;
constexpr const int &ok2 = *(&p.a + 1 - 1);
