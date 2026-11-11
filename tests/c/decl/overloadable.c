// clang's __attribute__((overloadable)) extension (spec/abi.md): in C,
// functions with the attribute may be overloaded, and are mangled as
// C++ functions.
//@ expect: run
//@ stdout: 1 2
int printf(const char *, ...);
__attribute__((overloadable)) int f(int x) { return 1; }
__attribute__((overloadable)) int f(double x) { return 2; }
int main(void) { printf("%d %d\n", f(1), f(1.0)); return 0; }
