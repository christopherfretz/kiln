// [dcl.constexpr]/6 (C++20): past the first read of a parameter, clang's
// never-constant walk does not check a local's value as a constant result:
// a pointer local holding a heap allocation, or the address of a
// temporary, is not a reason to report the function (WP-1163).
// spec/sema.md "Never constant".
//@ expect: compile-ok
//@ std: c++20
//@ args: -Wno-unused-value -Wno-c++23-extensions -Wno-dangling
//@ strict-diags
constexpr int f(int x) { x + 1; int *p = new int(1); delete p; return x; }
constexpr int g(int x) { x + 1; new int(1); return x; }
constexpr int h(int x) { int *p = nullptr; p = new int(2); delete p; return x; }
struct S { int v; };
constexpr int t(int x) { x + 1; const S *p = &static_cast<const S&>(S{1}); return x; }
