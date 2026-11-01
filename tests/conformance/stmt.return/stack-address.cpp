// [stmt.return], [basic.stc.auto]: returning the address of, a reference
// to, or a closure capturing by reference an automatic variable or parameter
// (clang's -Wreturn-stack-address, with its reference-variable and capture
// notes). Verified against clang 19.
//@ expect: compile-ok
//@ strict-diags
//@ diag: 46:32 warning "address of stack memory associated with local variable 'x' returned"
//@ diag: 47:35 warning "address of stack memory associated with local variable 'a' returned"
//@ diag: 48:31 warning "reference to stack memory associated with parameter 'p' returned"
//@ diag: 49:26 warning "address of stack memory associated with parameter 'p' returned"
//@ diag: 50:51 warning "reference to stack memory associated with local variable 'x' returned"
//@ diag: 51:55 warning "reference to stack memory associated with local variable 'x' returned"
//@ diag: 51:41 note "binding reference variable 'r' here"
//@ diag: 52:56 warning "reference to stack memory associated with local variable 'x' returned"
//@ diag: 52:41 note "binding reference variable 'r2' here"
//@ diag: 52:29 note "binding reference variable 'r' here"
//@ diag: 53:50 warning "address of stack memory associated with local variable 'x' returned"
//@ diag: 53:35 note "via initialization of variable 'p' here"
//@ diag: 54:49 warning "address of stack memory associated with local variable 'x' returned"
//@ diag: 54:54 warning "address of stack memory associated with local variable 'y' returned"
//@ diag: 55:39 warning "reference to stack memory associated with local variable 'x' returned"
//@ diag: 55:43 warning "reference to stack memory associated with local variable 'x' returned"
//@ diag: 56:35 warning "address of stack memory associated with local variable 'x' returned"
//@ diag: 57:36 warning "reference to stack memory associated with local variable 'a' returned"
//@ diag: 58:28 warning "reference to stack memory associated with local variable 's' returned"
//@ diag: 59:36 warning "address of stack memory associated with local variable 'a' returned"
//@ diag: 60:28 warning "address of stack memory associated with local variable 's' returned"
//@ diag: 61:44 warning "reference to stack memory associated with local variable 'c' returned"
//@ diag: 62:37 warning "address of stack memory associated with local variable 'x' returned"
//@ diag: 62:33 warning "left operand of comma operator has no effect"
//@ diag: 63:34 warning "address of stack memory associated with local variable 'x' returned"
//@ diag: 64:39 warning "address of stack memory associated with local variable 'x' returned"
//@ diag: 65:39 warning "reference to stack memory associated with local variable 'a' returned"
//@ diag: 66:33 warning "address of stack memory associated with local variable 'x' returned"
//@ diag: 66:33 note "captured by reference here"
//@ diag: 67:33 warning "address of stack memory associated with local variable 'x' returned"
//@ diag: 67:46 note "implicitly captured by reference due to use here"
//@ diag: 68:39 warning "address of stack memory associated with local variable 'x' returned"
//@ diag: 68:54 note "implicitly captured by reference due to use here"
//@ diag: 69:37 warning "address of stack memory associated with local variable 'x' returned"
//@ diag: 69:33 note "captured by reference via initialization of lambda capture 'r'"
//@ diag: 70:34 warning "address of stack memory associated with local variable 'z' returned"
//@ diag: 71:45 warning "address of stack memory associated with local variable 'x' returned"
//@ diag: 72:11 note "in instantiation of function template specialization 't1<int>' requested here"
struct S { int x; int a[2]; int* p; int& get() { return x; } };
int* f1() { int x = 0; return &x; }
int* f2() { int a[3] = {}; return a; }
const int& f3(int p) { return p; }
int* f4(int p) { return &p; }
int&& f5() { int x = 0; return static_cast<int&&>(x); }
const int& f6() { int x = 0; const int& r = x; return r; }
int& f7() { int x = 0; int& r = x; int& r2 = r; return r2; }
int* f8() { int x = 0; int* const p = &x; return p; }
int* f9(bool b) { int x = 0, y = 0; return b ? &x : &y; }
int& f10() { int x = 0; return true ? x : x; }
int* f11() { int x = 0; return &*&x; }
int& f12() { int a[3] = {}; return a[0]; }
int& f13() { S s{}; return s.x; }
int* f14() { int a[3] = {}; return a + 1; }
int* f15() { S s{}; return s.a; }
const int& f16() { const int c = 3; return c; }
int* f17() { int x = 0; return (0, &x); }
void* f18() { int x = 0; return &x; }
int* f19() { int x = 0; return (int*)&x; }
int& f20() { int a[2][2] = {}; return a[0][1]; }
auto l1() { int x = 0; return [&x] { return x; }; }
auto l2() { int x = 0; return [ & ] { return x; }; }
auto l3() { int x = 0, y = 1; return [&, y] { return x + y; }; }
auto l4() { int x = 0; return [&r = x] { return r; }; }
auto h = [] { int z = 0; return &z; };
template <class T> T* t1() { T x{}; return &x; }
int* u1 = t1<int>();
// Not diagnosed.
int* n1() { static int s; return &s; }
int& n2(int& p) { return p; }
int* n3() { int x = 0; int* p = &x; return p; }
const int& n4() { static int s; int& r = s; return r; }
int& n5(S& s) { return s.x; }
int& n6(S* s) { return s->x; }
int* n7(S s) { return s.p; }
int& n8(int* p) { return *p; }
int& n9(int (&a)[3]) { return a[1]; }
int& n10(int& p) { int& r = p; return r; }
const char* n11() { return "abc"; }
int& n12(S s) { return s.get(); }
auto n13(int& p) { return [&p] { return p; }; }
auto n14() { int x = 0; return [x] { return x; }; }
auto n15() { static int z; return [&] { return z; }; }
int* n16() { int x = 0; auto l = [&] { return &x; }; return l(); }
struct L { int* p; };
L n17() { int x = 0; return L{&x}; }
int& n18() { auto [a, b, c] = S{}; return a; }
