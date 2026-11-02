// [basic.extended.fp], [conv.double], [expr.arith.conv]: GCC's _Float32x
// and _Float64x at run time. They mangle as Itanium DF32x / DF64x and are
// passed exactly as double and long double: the C-linkage declaration below
// names the mangled symbol of f(_Float32x, _Float64x) with the twin types,
// so a wrong mangling fails to link and a wrong calling convention prints
// the wrong values. The inputs are volatile so nothing folds. The expected
// output is GCC 14's (g++ -std=c++23, x86-64 and AArch64; WP-380).
//@ expect: run
//@ std: c++23
//@ oracle: none
//@ skip(clang): clang 19 and 22 have no _Float32x or _Float64x (GCC is the reference)
//@ stdout: f 0x1.8p+0 2.5 g 0x1.5555555555555p-2 1
//@ stdout: mixed 0x1.8p+1 3.5 3.5
extern "C" int printf(const char *, ...);
volatile double one_half = 1.5;
volatile int three = 3;
void f(_Float32x a, _Float64x b) { printf("f %a %.1Lf ", (double)a, (long double)b); }
extern "C" void _Z1fDF32xDF64x(double, long double);
_Float32x g(_Float32x a) { return a / three; }
int main() {
    _Float32x x = one_half;
    _Float64x y = one_half + 1;
    _Z1fDF32xDF64x(x, y);
    _Float32x t = g(1);
    printf("g %a %d\n", (double)t, (long double)y > (_Float64x)x);
    _Float32x s = x + x;
    _Float64x u = y + 1;
    long double v = u;
    printf("mixed %a %.1Lf %.1Lf\n", (double)s, (long double)u, v);
}
