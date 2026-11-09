// [expr.const]/10.8: reading or modifying a volatile member (or an element
// of one, or a member of a nested object) is not a core constant
// expression even through a glvalue that drops the qualifier; only the
// accessed subobject's volatility counts. clang 22 (P2448R2, C++23) accepts
// a class with a volatile member in a constant expression; clang 19 calls it
// non-literal (volatile-member-access-19.cpp). WP-1019.
//@ std: c++26
//@ requires: clang>=22
//@ expect: compile-fail
//@ strict-diags
//@ diag: 41:15 error "constexpr variable 'k1' must be initialized by a constant expression"
//@ diag: 40:40 note "read of volatile member 'v' is not allowed in a constant expression"
//@ diag: 41:20 note "in call to 'm1()'"
//@ diag: 39:32 note "volatile member declared here"
//@ diag: 48:15 error "constexpr variable 'k2' must be initialized by a constant expression"
//@ diag: 44:11 note "read of volatile member 'v' is not allowed in a constant expression"
//@ diag: 48:20 note "in call to 'm2()'"
//@ diag: 39:32 note "volatile member declared here"
//@ diag: 51:15 error "constexpr variable 'k3' must be initialized by a constant expression"
//@ diag: 50:35 note "increment of volatile member 'v' is not allowed in a constant expression"
//@ diag: 51:20 note "in call to 'm3()'"
//@ diag: 39:32 note "volatile member declared here"
//@ diag: 54:15 error "constexpr variable 'k4' must be initialized by a constant expression"
//@ diag: 53:42 note "read of volatile member 'a' is not allowed in a constant expression"
//@ diag: 54:20 note "in call to 'm4()'"
//@ diag: 52:25 note "volatile member declared here"
//@ diag: 56:15 error "constexpr variable 'k5' must be initialized by a constant expression"
//@ diag: 55:37 note "assignment to volatile-qualified type 'volatile int' is not allowed in a constant expression"
//@ diag: 56:20 note "in call to 'm5()'"
//@ diag: 59:15 error "constexpr variable 'k6' must be initialized by a constant expression"
//@ diag: 58:29 note "assignment to volatile-qualified type 'volatile int' is not allowed in a constant expression"
//@ diag: 59:20 note "in call to 'm6()'"
//@ diag: 62:15 error "constexpr variable 'k7' must be initialized by a constant expression"
//@ diag: 61:31 note "assignment to volatile-qualified type 'volatile int' is not allowed in a constant expression"
//@ diag: 62:20 note "in call to 'm7()'"
//@ diag: 64:15 error "constexpr variable 'k8' must be initialized by a constant expression"
//@ diag: 63:36 note "assignment to volatile-qualified type 'volatile int' is not allowed in a constant expression"
//@ diag: 64:20 note "in call to 'm8()'"
struct S { int a; volatile int v; };
constexpr int m1() { S s{1, 2}; return const_cast<int &>(s.v); }
constexpr int k1 = m1();
constexpr int m2() {
  auto *p = new S{1, 2};
  int r = const_cast<int &>(p->v);
  delete p;
  return r;
}
constexpr int k2 = m2();
struct N { S s; };
constexpr int m3() { N n{{1, 2}}; ++const_cast<int &>(n.s.v); return 0; }
constexpr int k3 = m3();
struct T { volatile int a[2]; };
constexpr int m4() { T t{{1, 2}}; return const_cast<int *>(t.a)[1]; }
constexpr int k4 = m4();
constexpr int m5() { S s{1, 2}; s.v = 3; return 0; }
constexpr int k5 = m5();
union U { int i; volatile int v; };
constexpr int m6() { U u; u.v = 1; return 0; }
constexpr int k6 = m6();
union V { int i; struct { volatile int w; } s; volatile int a[2]; };
constexpr int m7() { V u; u.s.w = 1; return 0; }
constexpr int k7 = m7();
constexpr int m8() { V u; (u.a)[1] = 1; return 0; }
constexpr int k8 = m8();
