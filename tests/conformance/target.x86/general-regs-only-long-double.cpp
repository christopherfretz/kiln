// [basic.fundamental]/12 (long double is implementation-defined): on
// x86-64 `-mgeneral-regs-only` turns `x87` off, so the target has no
// `long double` (clang `HasLongDouble`), and clang's `checkTypeSupport`
// rejects each use of a declaration whose type is, takes or returns it,
// each definition of such a function (once per parameter and once for the
// return type) and each `long double` operand of a built-in binary
// operator. A reported declaration's later uses raise nothing more;
// unevaluated operands and `double` / `float` are fine. The target is
// pinned. spec/sema.md "Target type support" (WP-1804).
//@ expect: compile-fail
//@ strict-diags
//@ args: --target=x86_64-unknown-linux-gnu -mgeneral-regs-only
//@ diag: 37:30 error "'v' requires  'long double' type support, but target 'x86_64-unknown-linux-gnu' does not support it"
//@ diag: 34:13 note "'v' defined here"
//@ diag: 37:41 error "'m' requires  'long double' type support, but target 'x86_64-unknown-linux-gnu' does not support it"
//@ diag: 35:24 note "'m' defined here"
//@ diag: 38:15 error "'g' requires  'long double' type support, but target 'x86_64-unknown-linux-gnu' does not support it"
//@ diag: 33:13 note "'g' defined here"
//@ diag: 38:15 error "'g' requires  'long double' type support, but target 'x86_64-unknown-linux-gnu' does not support it"
//@ diag: 33:13 note "'g' defined here"
//@ diag: 39:39 error "'x' requires  'long double' type support, but target 'x86_64-unknown-linux-gnu' does not support it"
//@ diag: 39:27 note "'x' defined here"
//@ diag: 39:13 error "'f' requires  'long double' type support, but target 'x86_64-unknown-linux-gnu' does not support it"
//@ diag: 39:13 note "'f' defined here"
//@ diag: 39:13 error "'f' requires  'long double' type support, but target 'x86_64-unknown-linux-gnu' does not support it"
//@ diag: 39:13 note "'f' defined here"
//@ diag: 40:22 error "expression requires  'long double' type support, but target 'x86_64-unknown-linux-gnu' does not support it"
//@ diag: 40:22 error "expression requires  'long double' type support, but target 'x86_64-unknown-linux-gnu' does not support it"
//@ diag: 43:17 error "'tt<long double>' requires  'long double' type support, but target 'x86_64-unknown-linux-gnu' does not support it"
//@ diag: 42:22 note "'tt<long double>' defined here"
//@ diag: 43:17 error "'tt<long double>' requires  'long double' type support, but target 'x86_64-unknown-linux-gnu' does not support it"
//@ diag: 42:22 note "'tt<long double>' defined here"
long double g(long double);
long double v;
struct S { long double m; };
double d(double a, float b) { return a + b; }
int use() { S s; return (int)v + (int)s.m; }
void call() { g(2.0L); }
long double f(long double x) { return x + x; }
long double w = 1.0L + 2.0L;
int size = sizeof(g(1.0L)) + sizeof(long double);
template <class T> T tt(T t) { return t; }
int inst = (int)tt<long double>(1);
