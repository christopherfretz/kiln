//@ std: c++23
//@ requires: clang>=22
//@ strict-diags
//@ diag: 8:39 warning "structured binding packs are a C++2c extension"
// [dcl.struct.bind]/1 (P1061): before C++26 a structured binding pack is
// accepted as an extension (clang 22 `-Wc++26-extensions`).
struct P { int x, y; };
template <class T> int f(T t) { auto [...xs] = t; return sizeof...(xs); }
int a = f(P{});
