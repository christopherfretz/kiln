//@ std: c++23
//@ strict-diags
//@ diag: 8:49 warning "pack indexing is a C++2c extension"
//@ diag: 9:34 warning "pack indexing is a C++2c extension"
//@ diag: 10:35 warning "pack indexing is a C++2c extension"
// [expr.prim.pack.index], [dcl.type.pack.index] (P2662): before C++26 a
// pack index is an extension, warned at the pack's name (clang -Wc++26-extensions).
template <class... Ts> int f(Ts... ts) { return ts...[0]; }
template <class... Ts> using F = Ts...[0];
template <class... Ts> struct U { Ts...[0] m; };
int a = f(1);
F<int> b;
U<long> u;
