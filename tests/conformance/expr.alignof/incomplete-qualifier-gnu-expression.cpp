// [expr.alignof]/1 takes a type-id; `alignof` applied to an expression is a
// GNU extension, warned at the keyword after the operand is parsed, even when
// the operand is invalid (`alignof(F::x)` with `F` incomplete: "incomplete
// type 'F' named in nested name specifier", then the warning; a syntax error
// in the operand too), as clang's ext_alignof_expr. `__alignof__` takes an
// expression without the warning. A qualified incomplete type operand
// (`alignof(S::U)`) is "invalid application of 'alignof' to an incomplete
// type".
//@ expect: compile-fail
//@ strict-diags
//@ args: -ferror-limit=0
//@ diag: 36:21 error "incomplete type 'F' named in nested name specifier"
//@ diag: 35:8 note "forward declaration of 'F'"
//@ diag: 37:17 error "incomplete type 'F' named in nested name specifier"
//@ diag: 35:8 note "forward declaration of 'F'"
//@ diag: 37:9 warning "'alignof' applied to an expression is a GNU extension"
//@ diag: 38:21 error "incomplete type 'F' named in nested name specifier"
//@ diag: 35:8 note "forward declaration of 'F'"
//@ diag: 39:17 error "incomplete type 'F' named in nested name specifier"
//@ diag: 35:8 note "forward declaration of 'F'"
//@ diag: 39:9 warning "'alignof' applied to an expression is a GNU extension"
//@ diag: 41:9 warning "'alignof' applied to an expression is a GNU extension"
//@ diag: 43:21 error "expected expression"
//@ diag: 43:9 warning "'alignof' applied to an expression is a GNU extension"
//@ diag: 44:17 error "incomplete type 'F' named in nested name specifier"
//@ diag: 35:8 note "forward declaration of 'F'"
//@ diag: 44:9 warning "'alignof' applied to an expression is a GNU extension"
//@ diag: 45:28 error "incomplete type 'F' named in nested name specifier"
//@ diag: 35:8 note "forward declaration of 'F'"
//@ diag: 45:20 warning "'alignof' applied to an expression is a GNU extension"
//@ diag: 47:9 error "invalid application of '__alignof' to an incomplete type 'S::U'"
//@ diag: 46:19 note "forward declaration of 'S::U'"
//@ diag: 48:9 error "invalid application of 'alignof' to an incomplete type 'S::U'"
//@ diag: 46:19 note "forward declaration of 'S::U'"
struct F;
int a = __alignof__(F::x);
int b = alignof(F::x);
int c = __alignof__(F::v);
int d = alignof(F::v);
struct G { static int v; };
int e = alignof(G::v);
int f = __alignof__(G::v);
int h = alignof(1 + );
int i = alignof F::z;
using T = decltype(alignof(F::q));
struct S { struct U; };
int j = __alignof__(S::U);
int k = alignof(S::U);
