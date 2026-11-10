// [dcl.constexpr]/6 (C++20): clang's never-constant walk runs a loop over
// known locals to its end, however long, and reports what it finds after it
// (or its step limit, `-fconstexpr-steps`, at the statement it runs out at);
// kiln runs such a loop in one Oven job (WP-1163). spec/sema.md "Never
// constant".
//@ expect: compile-fail
//@ std: c++20
//@ args: -Wno-unused-value -Wno-c++23-extensions -ferror-limit=0
//@ strict-diags
//@ diag: 49:15 error "constexpr function never produces a constant expression"
//@ diag: 49:118 note "control flows through the definition of a static variable"
//@ diag: 50:15 error "constexpr function never produces a constant expression"
//@ diag: 50:94 note "control flows through the definition of a static variable"
//@ diag: 51:15 error "constexpr function never produces a constant expression"
//@ diag: 51:72 note "constexpr evaluation hit maximum step limit; possible infinite loop?"
//@ diag: 52:15 error "constexpr function never produces a constant expression"
//@ diag: 52:90 note "division by zero"
//@ diag: 53:15 error "constexpr function never produces a constant expression"
//@ diag: 53:86 note "division by zero"
//@ diag: 54:15 error "constexpr function never produces a constant expression"
//@ diag: 54:84 note "division by zero"
//@ diag: 56:15 error "constexpr function never produces a constant expression"
//@ diag: 56:115 note "division by zero"
//@ diag: 57:15 error "constexpr function never produces a constant expression"
//@ diag: 57:180 note "division by zero"
//@ diag: 58:15 error "constexpr function never produces a constant expression"
//@ diag: 58:40 note "constexpr evaluation hit maximum step limit; possible infinite loop?"
//@ diag: 59:36 warning "for loop has empty body"
//@ diag: 59:36 note "put the semicolon on a separate line to silence this warning"
//@ diag: 59:15 error "constexpr function never produces a constant expression"
//@ diag: 59:36 note "constexpr evaluation hit maximum step limit; possible infinite loop?"
//@ diag: 60:15 error "constexpr function never produces a constant expression"
//@ diag: 60:43 note "constexpr evaluation hit maximum step limit; possible infinite loop?"
//@ diag: 61:15 error "constexpr function never produces a constant expression"
//@ diag: 61:52 note "constexpr evaluation hit maximum step limit; possible infinite loop?"
//@ diag: 62:15 error "constexpr function never produces a constant expression"
//@ diag: 62:103 note "division by zero"
//@ diag: 65:15 error "constexpr function never produces a constant expression"
//@ diag: 65:100 note "division by zero"
//@ diag: 68:15 error "constexpr function never produces a constant expression"
//@ diag: 68:72 note "constexpr evaluation hit maximum step limit; possible infinite loop?"
//@ diag: 70:15 error "constexpr function never produces a constant expression"
//@ diag: 70:77 note "division by zero"
//@ diag: 71:15 error "constexpr function never produces a constant expression"
//@ diag: 71:53 note "constexpr evaluation hit maximum step limit; possible infinite loop?"
//@ diag: 73:15 error "constexpr function never produces a constant expression"
//@ diag: 73:145 note "value 2147483648 is outside the range of representable values of type 'int'"

constexpr int l1(int x) { int s = 0; for (int k = 0; k < 10000; ++k) s += k; if (s != 49995000) return 0; static int i = 0; return i; }
constexpr int l2(int x) { int s = 0; for (int k = 0; k < 100000; ++k) { s += 1; } static int i = 0; return i; }
constexpr int l3(int x) { int s = 0; for (int k = 0; k < 2000000; ++k) { s += 1; } return s; }
constexpr int l4(int x) { int s = 0; for (int k = 0; k < 5000; ++k) { s += k; } return 1 / (s - 12497500); }
constexpr int l5(int x) { int s = 0; int k = 0; while (k < 300000) { ++k; } return 1 / (k - 300000); }
constexpr int l6(int x) { int s = 0; for (int k = 0; k < 600000; ++k) { } return 1 / s; }
constexpr int g(int);
constexpr int n1(int x) { int s = 0; for (int i = 0; i < 300; ++i) for (int j = 0; j < 300; ++j) s += j; return 1 / (s - 13455000); }
constexpr int n2(int x) { int s = 0; for (int i = 0; i < 2000; ++i) { switch (i % 3) { case 0: s += 1; break; case 1: continue; default: break; } if (i == 1500) break; } return 1 / (s - 501); }
constexpr int n3(int x) { while (true) {} return 0; }
constexpr int n4(int x) { for (;;) ; return 0; }
constexpr int n5(int x) { int s = 0; do { ++s; } while (s < 3000000); return s; }
constexpr int n6(int x) { int s = 0; while (s < 1) { } return s; }
constexpr int n7(int x) { int a[16] = {}; int *p = a; for (int i = 0; i < 16; ++i) *p++ = i; return 1 / (a[15] - 15); }
constexpr int n8(int x) { int s = 0; for (int i = 0; i < 100000; ++i) s += x; return 1 / s; }
constexpr int n9(int x) { int s = 0; for (int i = 0; i < 100000; ++i) s += g(i); return 1 / (s - s); }
constexpr int n10(int x) { int s = 0; for (int i = 0; i < 100000; ++i) { int u; s += 1; } return 1 / (s - 100000); }
constexpr int n11(int x) { int *p = nullptr; for (int i = 0; i < 10; ++i) { int k = i; p = &k; } return 1 / x; }
constexpr int n12(int x) { int s = 0; for (int i = 0; i < 100000; ++i) { s += 1; } x = 0; for (int i = 0; i < 1000000; ++i) {} return s; }
constexpr int n13(int x) { int s = 0; for (int i = 0; i < 524287; ++i) { s += 1; } for (int i = 0; i < 10; ++i) {} return 1 / (s - 524287); }
constexpr int n14(int x) { int s = 0; for (int i = 0; i < 524286; ++i) { } return 1 / (s - 1); }
constexpr int n15(int x) { int s = 0; for (int i = 0; i < 10; ++i) { s += 1 / (i - 7); } return s; }
constexpr int n16(int x) { int s = 0; while (s < 5) { s += 1; if (s == 3) s = 0; } return s; }
constexpr int n17(int x) { int s = 0; for (int i = 0; i < 1000; ++i) { s += i; [[assume(true)]]; } return 1 / s; }
constexpr int n19(int x) { unsigned u = 0; for (int i = 0; i < 70000; ++i) u = u * 3 + 1; int big = 2147483647; for (int i = 0; i < 2; ++i) big += 1; return u; }
