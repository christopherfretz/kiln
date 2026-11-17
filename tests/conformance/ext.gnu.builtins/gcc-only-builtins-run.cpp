// GCC builtins clang 19/22 lack: __builtin_ffsg, __builtin_clrsbg (a
// signed integer of any width), __builtin_parityg (an unsigned one),
// __builtin_assoc_barrier (its operand's value; scalars, pointers and
// vectors) and __builtin_speculation_safe_value (its first operand, the
// second evaluated for its side effects only), at run time through
// volatile operands and in templates, and __builtin_puts (the library
// puts). Found running GCC's C++ testsuite. Compared with GCC 16.1.
// WP-1552.
//@ std: c++20
//@ expect: run
//@ oracle: gcc
//@ skip(clang): clang 19/22 have none of these builtins
//@ strict-diags
//@ stdout: 1 31 1 8 15 1 0 | 3 1e+20 2 4.5 | 5 6 1 | hi
extern "C" int printf(const char *, ...);

typedef float v4f __attribute__((vector_size(16)));

int arr[4] = {10, 20, 30, 40};
int side;
long bump() { return ++side; }

template <class T> T rounded(T a, T b) { return __builtin_assoc_barrier(a + b) - b; }
template <class T> int ffs(T x) { return __builtin_ffsg(x); }

int main(int argc, char **) {
    volatile int m1 = -argc;
    volatile signed char c = -128;
    volatile short s = -1;
    volatile unsigned __int128 w = (unsigned __int128)1 << 100;
    printf("%d %d %d %d %d %d %d | ", __builtin_ffsg(m1), __builtin_clrsbg(m1),
           __builtin_parityg((unsigned)m1 >> 1), ffs(c), __builtin_clrsbg(s),
           __builtin_parityg(w), __builtin_ffsg((long long)0));
    volatile double big = 1e20;
    v4f v = {1, 2, 3, 4};
    v4f r = __builtin_assoc_barrier(v * 1.5f);
    int *p = __builtin_assoc_barrier(arr + 1);
    printf("%d %g %d %g | ", rounded(argc + 2, 4), rounded((double)big, 1.0), *p / 10,
           (double)r[2]);
    long l = __builtin_speculation_safe_value((long)argc + 4, bump());
    int *q = __builtin_speculation_safe_value(arr + 2);
    printf("%ld %d %d | ", l, *q / 5, side);
    __builtin_puts("hi");
}
