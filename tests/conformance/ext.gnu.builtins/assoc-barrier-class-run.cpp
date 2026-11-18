// GCC's __builtin_assoc_barrier on a class operand: GCC 16.1 accepts any
// class and passes the object through without a copy (a class with a
// deleted copy constructor is accepted; the call names the operand, so a
// member of it can be assigned). kiln makes the call a glvalue of the
// operand's type: an lvalue for an lvalue, an xvalue for an xvalue or a
// (materialized) prvalue; a copy happens only where the result
// initializes another object. Not compared with GCC: GCC 16.1 ICEs
// emitting this for a class with a user copy constructor. Found running
// GCC's C++ testsuite (WP-1552 left it out). WP-1611.
//@ std: c++20
//@ expect: run
//@ oracle: none
//@ skip(clang): clang 19/22 have no __builtin_assoc_barrier
//@ strict-diags
//@ stdout: copy 101 | 7 8 | move 1002 | 9 | 4 6
extern "C" int printf(const char *, ...);

struct S {
    int v;
    S(int x) : v(x) {}
    S(const S &o) : v(o.v + 100) { printf("copy "); }
    S(S &&o) : v(o.v + 1000) { printf("move "); }
};
struct NoCopy {
    int v = 7;
    NoCopy() = default;
    NoCopy(const NoCopy &) = delete;
};
struct T {
    int v;
};

template <class X> X pass(X x) { return __builtin_assoc_barrier(x); }

constexpr T ct{4};
constexpr T cd = __builtin_assoc_barrier(ct);
static_assert(cd.v == 4);
constexpr int assign_through() {
    T t{5};
    __builtin_assoc_barrier(t).v = 6;
    return t.v;
}
static_assert(assign_through() == 6);
static_assert(__is_same(decltype(__builtin_assoc_barrier(ct)), const T &));
static_assert(__is_same(decltype(__builtin_assoc_barrier(T{1})), T &&));

int main() {
    S a(1);
    S b = __builtin_assoc_barrier(a);
    printf("%d | ", b.v);
    NoCopy n;
    int r = __builtin_assoc_barrier(n).v;
    __builtin_assoc_barrier(n).v = 8;
    printf("%d %d | ", r, n.v);
    S c = __builtin_assoc_barrier(S(2));
    printf("%d | ", c.v);
    printf("%d | ", pass(T{9}).v);
    printf("%d %d\n", cd.v, assign_through());
}
