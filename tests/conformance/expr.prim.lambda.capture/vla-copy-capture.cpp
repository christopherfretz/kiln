//@ expect: compile-fail
//@ strict-diags
//@ args: -std=gnu++20
//@ diag: 15:35 error "variable-sized object may not be initialized"
//@ diag: 16:35 error "variable-sized object may not be initialized"
//@ diag: 17:39 error "variable-sized object may not be initialized"
//@ diag: 18:34 error "no matching function for call to 'tf'"
//@ diag: 14:24 note "candidate template ignored: invalid explicitly-specified argument for template parameter 'T'"
// [expr.prim.lambda.capture]/10 with variable length arrays (GNU
// extension): a closure member cannot be copy-initialized from a variable
// length array, implicitly (at the capture default) or explicitly (at the
// capture), and a variable length array type is no template argument
// (clang 22 and GCC 16.1 agree). Found running GCC's C++ testsuite.
template <class T> int tf(T) { return sizeof(T); }
int r1(int n) { int a[n]; return [=] { return a[0]; }(); }
int r2(int n) { int a[n]; return [a] { return a[0]; }(); }
int r3(int n) { int a[n]; return [&n, a] { return a[0]; }(); }
int r4(int n) { int a[n]; return tf<decltype(a)>(a); }
int ok(int n) { int a[n]; a[0] = 1; return [&] { return tf(a); }(); }
