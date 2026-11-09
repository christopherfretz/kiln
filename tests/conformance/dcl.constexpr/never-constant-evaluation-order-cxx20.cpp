// [dcl.constexpr]/6 (C++20): an expression that names both a parameter and a
// local is evaluated in clang's order: a failure on a local before the first
// read of the parameter is reported; past it, clang fails quietly on every
// local but still reports a failure of an operand that names none
// (WP-1163). spec/sema.md "Never constant".
//@ expect: compile-fail
//@ std: c++20
//@ args: -Wno-unused-value -Wno-c++23-extensions -ferror-limit=0
//@ strict-diags
//@ diag: 55:15 error "constexpr function never produces a constant expression"
//@ diag: 55:47 note "division by zero"
//@ diag: 59:15 error "constexpr function never produces a constant expression"
//@ diag: 59:60 note "read of dereferenced one-past-the-end pointer is not allowed in a constant expression"
//@ diag: 64:15 error "constexpr function never produces a constant expression"
//@ diag: 64:56 note "division by zero"
//@ diag: 66:15 error "constexpr function never produces a constant expression"
//@ diag: 66:48 note "division by zero"
//@ diag: 67:55 warning "division by zero is undefined"
//@ diag: 67:15 error "constexpr function never produces a constant expression"
//@ diag: 67:55 note "division by zero"
//@ diag: 69:15 error "constexpr function never produces a constant expression"
//@ diag: 69:58 note "value 2147483648 is outside the range of representable values of type 'int'"
//@ diag: 70:15 error "constexpr function never produces a constant expression"
//@ diag: 70:63 note "division by zero"
//@ diag: 72:15 error "constexpr function never produces a constant expression"
//@ diag: 72:52 note "division by zero"
//@ diag: 75:15 error "constexpr function never produces a constant expression"
//@ diag: 75:47 note "division by zero"
//@ diag: 76:15 error "constexpr function never produces a constant expression"
//@ diag: 76:61 note "cannot refer to element 5 of array of 2 elements in a constant expression"
//@ diag: 78:40 warning "division by zero is undefined"
//@ diag: 78:15 error "constexpr function never produces a constant expression"
//@ diag: 78:40 note "division by zero"
//@ diag: 79:52 warning "division by zero is undefined"
//@ diag: 79:15 error "constexpr function never produces a constant expression"
//@ diag: 79:52 note "division by zero"
//@ diag: 80:15 error "constexpr function never produces a constant expression"
//@ diag: 80:62 note "division by zero"
//@ diag: 81:15 error "constexpr function never produces a constant expression"
//@ diag: 81:60 note "cannot refer to element 3 of array of 2 elements in a constant expression"
//@ diag: 82:15 error "constexpr function never produces a constant expression"
//@ diag: 82:45 note "division by zero"
//@ diag: 83:15 error "constexpr function never produces a constant expression"
//@ diag: 83:63 note "division by zero"
//@ diag: 84:58 warning "division by zero is undefined"
//@ diag: 84:15 error "constexpr function never produces a constant expression"
//@ diag: 84:58 note "division by zero"
//@ diag: 85:55 warning "division by zero is undefined"
//@ diag: 85:15 error "constexpr function never produces a constant expression"
//@ diag: 85:55 note "division by zero"
//@ diag: 88:15 error "constexpr function never produces a constant expression"
//@ diag: 88:49 note "division by zero"

constexpr int a1(int x) { int k = 0; return x + 1 / k; }
constexpr int a2(int x) { int k = 0; return 1 / k + x; }
constexpr int a3(int x) { int k = 0; return (x + 1) / k; }
constexpr int a4(int x) { int k = 0; return x / k; }
constexpr int a5(int x) { int k = 0; int a[2] = {}; return a[x] + a[k + 2]; }
constexpr int a6(int x) { int k = 0; int a[2] = {}; return a[k + 2] + a[x]; }
constexpr int a7(int x) { int k = 0; if (x + 1 / k) {} return 0; }
constexpr int a8(int x) { int k = 0; int j = x + 1 / k; return j; }
constexpr int a9(int x) { int k = 0; x + 1 / k; return 0; }
constexpr int a10(int x) { int k = 0; return x ? 1 / k : 2 / k; }
constexpr int a11(int x) { int k = 0; return k ? x : 1 / k; }
constexpr int a12(int x) { int k = 1; return x && 1 / (k - 1); }
constexpr int a13(int x) { int k = 0; return 1 / k && x; }
constexpr int a14(int x) { int k = 0; k = x; return 1 / 0; }
constexpr int a15(int x) { int k = 2147483647; return x + (k + 1); }
constexpr int a16(int x) { int k = 2147483647; return (k + 1) + x; }
constexpr int a17(int x) { int k = 0; int a[2] = {}; a[x] = 1 / k; return 0; }
constexpr int a18(int x) { int k = 0; k += x; return 0; }
constexpr int a19(int x) { int k = 0; return (x, 1 / k); }
constexpr int a20(int x) { int k = 0; return 1 / (k * x); }
constexpr int a21(int x) { int k = 0; return x + k++ + 1 / k; }
constexpr int a22(int x) { int k = 0; return 1/k + 1/(k+0) + x; }
constexpr int a23(int x) { int k = 0; int a[2] = {}; return a[k+5] + 1/k + x; }
constexpr int a24(int x) { int k = 0; int a[2] = {}; return x + a[k+5] + 1/k; }
constexpr int b1(int x) { return x + 1 / 0; }
constexpr int b2(int x) { int k = 0; return x + (1 / 0); }
constexpr int b3(int x) { int k = 0; int a[1] = {}; a[0] = 1 / k; a[x] = 2; return 0; }
constexpr int b4(int x) { int k = 0; int a[2] = {}; a[x] = a[k + 3]; return 0; }
constexpr int b5(int x) { int k = 0; k += 1 / k; return 0; }
constexpr int b6(int x) { int k = 0; int j = (x, 0); return 1 / k; }
constexpr int b7(int x) { int k = 0; k = k + x; return 1 / 0; }
constexpr int b8(int x) { int k = 0; return k + x + 1 / 0; }
constexpr int b9(int x, int y) { int k = 0; return (x + y) + 1/k; }
constexpr int b10(int x) { int k = 0; int a[2] = {}; return a[x] + a[k+3]; }
constexpr int b11(int x) { int k = 0; return (1 / k) * x; }
constexpr int b12(int x) { int k = 0; return x * (1 / k); }
constexpr int b13(int x) { int k = 0; return -x + -(1 / k); }
constexpr int b14(int x) { int k = 0; return x < 1 / k; }
constexpr int b15(int x) { int k = 0; return x == 0 ? 1 : 1 / k; }
constexpr int b16(int x) { int k = 0; k = x; return 1 / k; }
