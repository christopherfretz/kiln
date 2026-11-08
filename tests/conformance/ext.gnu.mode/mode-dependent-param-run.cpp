//@ expect: run
//@ strict-diags
//@ xfail(kiln): WP-807 follow-up, a mode on a parameter of dependent type is a hard error (the parameter's type is substituted with its function's)
//@ stdout: 8
// GCC mode on a function parameter of dependent type: clang applies it to
// the instantiated parameter.
#include <cstdio>

template <class T> int f(T x __attribute__((mode(DI)))) { return sizeof(x); }

int main() { std::printf("%d\n", f<char>(1)); }
