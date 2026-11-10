// [dcl.constexpr]/6 (C++20): a null pointer local dereferenced, and a
// comparison of pointers to two locals, on the never-constant walk's known
// path (WP-1163), in clang 22's wording (the clang 19 shape is
// never-constant-pointer-null-clang19.cpp). spec/sema.md "Never constant".
//@ requires: clang>=22
//@ expect: compile-fail
//@ std: c++20
//@ args: -Wno-unused-value -Wno-c++23-extensions -ferror-limit=0
//@ strict-diags
//@ diag: 26:15 error "constexpr function never produces a constant expression"
//@ diag: 26:60 note "dereferencing a null pointer is not allowed in a constant expression"
//@ diag: 27:15 error "constexpr function never produces a constant expression"
//@ diag: 27:48 note "dereferencing a null pointer is not allowed in a constant expression"
//@ diag: 28:15 error "constexpr function never produces a constant expression"
//@ diag: 28:80 note "dereferencing a null pointer is not allowed in a constant expression"
//@ diag: 29:15 error "constexpr function never produces a constant expression"
//@ diag: 29:59 note "dereferencing a null pointer is not allowed in a constant expression"
//@ diag: 30:15 error "constexpr function never produces a constant expression"
//@ diag: 30:65 note "dereferencing a null pointer is not allowed in a constant expression"
//@ diag: 31:15 error "constexpr function never produces a constant expression"
//@ diag: 31:82 note "comparison between pointers to unrelated objects '&a' and '&b' has unspecified value"
//@ diag: 32:15 error "constexpr function never produces a constant expression"
//@ diag: 32:69 note "dereferencing a null pointer is not allowed in a constant expression"

struct S { int v; int *q; };
constexpr int null_deref(int x) { int *p = nullptr; return *p; }
constexpr int null_deref2(int x) { int *p = 0; *p = 1; return 0; }
constexpr int ptr_to_param_null(int x) { int *p = &x; int *q = nullptr; return *q; }
constexpr int ptr_struct(int x) { S s{1, nullptr}; return *s.q; }
constexpr int const_ptr(int x) { const int *p = nullptr; return *p; }
constexpr int ptr_cmp(int x) { int a = 0, b = 0; int *p = &a; int *q = &b; if (p < q) {} static int i = 0; return i; }
constexpr int null_after_param(int x) { int *p = nullptr; x; return *p; }
