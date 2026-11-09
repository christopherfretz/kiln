// [expr.const]/10.8: an lvalue-to-rvalue conversion of a volatile object
// (a volatile local, array, temporary or heap object) is not a core constant
// expression, even through a glvalue that drops the qualifier; neither is a
// modification of one ([expr.const]/10.16 via clang's volatile-object rule).
// A write or increment through a volatile-qualified glvalue fails on its type
// (clang `note_constexpr_access_volatile_type`). WP-1019.
//@ std: c++26
//@ expect: compile-fail
//@ strict-diags
//@ diag: 51:15 error "constexpr variable 'k1' must be initialized by a constant expression"
//@ diag: 50:49 note "read of volatile object 'v' is not allowed in a constant expression"
//@ diag: 51:20 note "in call to 'a1()'"
//@ diag: 50:35 note "volatile object declared here"
//@ diag: 53:15 error "constexpr variable 'k2' must be initialized by a constant expression"
//@ diag: 52:57 note "read of volatile object 'a' is not allowed in a constant expression"
//@ diag: 53:20 note "in call to 'a2()'"
//@ diag: 52:35 note "volatile object declared here"
//@ diag: 55:15 error "constexpr variable 'k3' must be initialized by a constant expression"
//@ diag: 54:55 note "read of volatile object 'c' is not allowed in a constant expression"
//@ diag: 55:20 note "in call to 'a3()'"
//@ diag: 54:41 note "volatile object declared here"
//@ diag: 62:15 error "constexpr variable 'k4' must be initialized by a constant expression"
//@ diag: 58:11 note "read of volatile temporary is not allowed in a constant expression"
//@ diag: 62:20 note "in call to 'a4()'"
//@ diag: 64:15 error "constexpr variable 'k5' must be initialized by a constant expression"
//@ diag: 63:51 note "read of volatile temporary is not allowed in a constant expression"
//@ diag: 64:20 note "in call to 'a5()'"
//@ diag: 63:41 note "volatile temporary created here"
//@ diag: 66:15 error "constexpr variable 'k6' must be initialized by a constant expression"
//@ diag: 65:63 note "assignment to volatile object 'v' is not allowed in a constant expression"
//@ diag: 66:20 note "in call to 'a6()'"
//@ diag: 65:35 note "volatile object declared here"
//@ diag: 68:15 error "constexpr variable 'k7' must be initialized by a constant expression"
//@ diag: 67:44 note "assignment to volatile-qualified type 'volatile int' is not allowed in a constant expression"
//@ diag: 68:20 note "in call to 'a7()'"
//@ diag: 69:42 warning "increment of object of volatile-qualified type 'volatile int' is deprecated [-Wdeprecated-volatile]"
//@ diag: 70:15 error "constexpr variable 'k8' must be initialized by a constant expression"
//@ diag: 69:42 note "increment of volatile-qualified type 'volatile int' is not allowed in a constant expression"
//@ diag: 70:20 note "in call to 'a8()'"
//@ diag: 71:43 warning "decrement of object of volatile-qualified type 'volatile int' is deprecated [-Wdeprecated-volatile]"
//@ diag: 72:15 error "constexpr variable 'k9' must be initialized by a constant expression"
//@ diag: 71:43 note "decrement of volatile-qualified type 'volatile int' is not allowed in a constant expression"
//@ diag: 72:20 note "in call to 'a9()'"
//@ diag: 74:15 error "constexpr variable 'k10' must be initialized by a constant expression"
//@ diag: 73:45 note "assignment to volatile-qualified type 'volatile int' is not allowed in a constant expression"
//@ diag: 74:21 note "in call to 'a10()'"
//@ diag: 76:15 error "constexpr variable 'k11' must be initialized by a constant expression"
//@ diag: 75:57 note "assignment to volatile-qualified type 'volatile int' is not allowed in a constant expression"
//@ diag: 76:21 note "in call to 'a11()'"
constexpr int a1() { volatile int v = 1; return const_cast<int &>(v); }
constexpr int k1 = a1();
constexpr int a2() { volatile int a[2] = {1, 2}; return const_cast<int *>(a)[1]; }
constexpr int k2 = a2();
constexpr int a3() { const volatile int c = 3; return const_cast<const int &>(c); }
constexpr int k3 = a3();
constexpr int a4() {
  auto *p = new volatile int[2]{4, 5};
  int r = const_cast<int *>(p)[1];
  delete[] p;
  return r;
}
constexpr int k4 = a4();
constexpr int a5() { volatile int &&r = 5; return const_cast<int &>(r); }
constexpr int k5 = a5();
constexpr int a6() { volatile int v = 1; const_cast<int &>(v) = 2; return 0; }
constexpr int k6 = a6();
constexpr int a7() { volatile int v = 1; v = 2; return 0; }
constexpr int k7 = a7();
constexpr int a8() { volatile int v = 1; ++v; return 0; }
constexpr int k8 = a8();
constexpr int a9() { volatile int v = 1; v--; return 0; }
constexpr int k9 = a9();
constexpr int a10() { volatile int v = 1; v += 2; return 0; }
constexpr int k10 = a10();
constexpr int a11() { int x = 1; volatile int &r = x; r = 3; return x; }
constexpr int k11 = a11();
