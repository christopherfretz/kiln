// [dcl.constexpr]/6 (C++20): clang's never-constant walk knows the value of a
// pointer local (null, the address of a local, a global or a string literal,
// pointer arithmetic on those) and reports what it finds through it; a read
// past a scope's end or of a pointer to a temporary or a parameter fails
// quietly (WP-1163). spec/sema.md "Never constant".
//@ expect: compile-fail
//@ std: c++20
//@ args: -Wno-unused-value -Wno-c++23-extensions -ferror-limit=0
//@ strict-diags
//@ diag: 53:15 error "constexpr function never produces a constant expression"
//@ diag: 53:93 note "control flows through the definition of a static variable"
//@ diag: 54:15 error "constexpr function never produces a constant expression"
//@ diag: 54:91 note "control flows through the definition of a static variable"
//@ diag: 55:15 error "constexpr function never produces a constant expression"
//@ diag: 55:71 note "read of dereferenced one-past-the-end pointer is not allowed in a constant expression"
//@ diag: 56:15 error "constexpr function never produces a constant expression"
//@ diag: 56:102 note "control flows through the definition of a static variable"
//@ diag: 57:15 error "constexpr function never produces a constant expression"
//@ diag: 57:68 note "cannot refer to element 4 of array of 3 elements in a constant expression"
//@ diag: 58:15 error "constexpr function never produces a constant expression"
//@ diag: 58:56 note "read of non-const variable 'g' is not allowed in a constant expression"
//@ diag: 59:15 error "constexpr function never produces a constant expression"
//@ diag: 59:74 note "control flows through the definition of a static variable"
//@ diag: 60:15 error "constexpr function never produces a constant expression"
//@ diag: 60:83 note "control flows through the definition of a static variable"
//@ diag: 61:15 error "constexpr function never produces a constant expression"
//@ diag: 61:75 note "control flows through the definition of a static variable"
//@ diag: 62:15 error "constexpr function never produces a constant expression"
//@ diag: 62:55 note "cannot perform pointer arithmetic on null pointer"
//@ diag: 66:15 error "constexpr function never produces a constant expression"
//@ diag: 66:107 note "control flows through the definition of a static variable"
//@ diag: 67:15 error "constexpr function never produces a constant expression"
//@ diag: 67:62 note "cannot access field of null pointer"
//@ diag: 68:15 error "constexpr function never produces a constant expression"
//@ diag: 68:100 note "control flows through the definition of a static variable"
//@ diag: 69:15 error "constexpr function never produces a constant expression"
//@ diag: 69:81 note "control flows through the definition of a static variable"
//@ diag: 70:15 error "constexpr function never produces a constant expression"
//@ diag: 70:61 note "cannot refer to element 5 of array of 3 elements in a constant expression"
//@ diag: 71:65 warning "temporary whose address is used as value of local variable 'p' will be destroyed at the end of the full-expression"
//@ diag: 75:15 error "constexpr function never produces a constant expression"
//@ diag: 75:101 note "control flows through the definition of a static variable"
//@ diag: 76:15 error "constexpr function never produces a constant expression"
//@ diag: 76:63 note "'f' evaluates to a null function pointer"
//@ diag: 77:15 error "constexpr function never produces a constant expression"
//@ diag: 77:61 note "cast from 'void *' is not allowed in a constant expression"
//@ diag: 79:15 error "constexpr function never produces a constant expression"
//@ diag: 79:58 note "read of dereferenced null pointer is not allowed in a constant expression"

int g;
int ga[3];
struct S { int v; int *q; };
constexpr int local_addr(int x) { int k = 0; int *p = &k; *p = 2; if (k == 2) {} static int i = 0; return i; }
constexpr int local_addr_read(int x) { int k = 3; int *p = &k; if (*p == 3) {} static int i = 0; return i; }
constexpr int past_end(int x) { int a[2] = {}; int *p = a + 2; return *p; }
constexpr int arith(int x) { int a[3] = {1,2,3}; int *p = a; p++; p += 1; if (*p == 3) {} static int i = 0; return i; }
constexpr int arith_oob(int x) { int a[3] = {1,2,3}; int *p = a; p += 4; return 0; }
constexpr int global_addr(int x) { int *p = &g; return *p; }
constexpr int global_addr_cmp(int x) { int *p = &g; if (p) {} static int i = 0; return i; }
constexpr int global_arr(int x) { int *p = ga + 1; if (p == ga + 1) {} static int i = 0; return i; }
constexpr int null_check(int x) { int *p = nullptr; if (!p) {} static int i = 0; return i; }
constexpr int null_arith(int x) { int *p = nullptr; p + 1; return 0; }
constexpr int ptr_param(int *x) { int *p = x; return *p; }
constexpr int ptr_to_param(int x) { int *p = &x; return *p; }
constexpr int ptr_dangling(int x) { int *p; { int k = 0; p = &k; } return *p; }
constexpr int ptr_ptr(int x) { int k = 0; int *p = &k; int **pp = &p; **pp = 5; if (k == 5) {} static int i = 0; return i; }
constexpr int null_member(int x) { S *p = nullptr; return p->v; }
constexpr int ptr_sub(int x) { int a[3]; int *p = a; int *q = a + 2; if (q - p == 2) {} static int i = 0; return i; }
constexpr int lit(int x) { const char *p = "ab"; if (p[1] == 'b') {} static int i = 0; return i; }
constexpr int lit_oob(int x) { const char *p = "ab"; return p[5]; }
constexpr int temp(int x) { const S *p = &static_cast<const S&>(S{1}); return p->v; }
constexpr int dangling(int x) { int *p; { int k = 0; p = &k; } return *p; }
constexpr int dangling2(int x) { int *p = nullptr; for (int k = 0; k < 1; ++k) p = &k; return *p; }
constexpr int uninit_ptr(int x) { int *p; return *p; }
constexpr int ptr_eq(int x) { int a = 0, b = 0; int *p = &a; int *q = &b; if (p == q) {} static int i = 0; return i; }
constexpr int ptr_fn(int x) { int (*f)(int) = nullptr; return f(1); }
constexpr int ptr_void(int x) { void *v = nullptr; int *p = static_cast<int*>(v); return *p; }
constexpr int ptr_from_param_then(int x, int *y) { int *p = y; int *q = nullptr; return *q; }
constexpr int null_arr(int x) { int *p = nullptr; return p[0]; }
constexpr int null_after_side(int x) { int *p = nullptr; x + 1; return *p; }
