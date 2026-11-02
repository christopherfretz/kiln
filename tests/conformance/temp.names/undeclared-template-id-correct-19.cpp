// [temp.names]/3, [basic.lookup.general]: under clang 19 an undeclared
// template-id whose name finds exactly one non-member function by
// qualified lookup is a delayed typo corrected to the qualified name
// ("did you mean '::val'?", template arguments dropped) when the rebuilt
// expression is valid at the correction point (expression statement,
// return, initializer, condition, decltype). Otherwise -- invalid rebuilt
// expression, initializer type mismatch, call argument, comma operands
// corrected together -- it stays "use of undeclared identifier".
//@ requires: clang<22
//@ expect: compile-fail
//@ strict-diags
//@ args: -ferror-limit=0
//@ diag: 73:12 error "use of undeclared identifier 'val'; did you mean '::val'?"
//@ diag: 68:5 note "'::val' declared here"
//@ diag: 74:26 error "use of undeclared identifier 'val'; did you mean '::val'?"
//@ diag: 68:5 note "'::val' declared here"
//@ diag: 75:17 error "use of undeclared identifier 'val'; did you mean '::val'?"
//@ diag: 68:5 note "'::val' declared here"
//@ diag: 76:18 error "use of undeclared identifier 'val'; did you mean '::val'?"
//@ diag: 68:5 note "'::val' declared here"
//@ diag: 77:22 error "use of undeclared identifier 'val'; did you mean '::val'?"
//@ diag: 68:5 note "'::val' declared here"
//@ diag: 78:19 error "use of undeclared identifier 'val'; did you mean '::val'?"
//@ diag: 68:5 note "'::val' declared here"
//@ diag: 79:19 error "use of undeclared identifier 'val'; did you mean '::val'?"
//@ diag: 68:5 note "'::val' declared here"
//@ diag: 80:35 error "use of undeclared identifier 'fn'; did you mean '::N::fn'?"
//@ diag: 70:19 note "'::N::fn' declared here"
//@ diag: 81:21 error "use of undeclared identifier 'val'; did you mean '::val'?"
//@ diag: 68:5 note "'::val' declared here"
//@ diag: 82:16 error "use of undeclared identifier 'val'; did you mean '::val'?"
//@ diag: 68:5 note "'::val' declared here"
//@ diag: 83:20 error "expected ';' after expression"
//@ diag: 83:12 error "use of undeclared identifier 'val'; did you mean '::val'?"
//@ diag: 68:5 note "'::val' declared here"
//@ diag: 83:21 warning "expression result unused"
//@ diag: 84:26 error "invalid operands to binary expression ('const char[2]' and 'int')"
//@ diag: 84:12 error "use of undeclared identifier 'val'; did you mean '::val'?"
//@ diag: 68:5 note "'::val' declared here"
//@ diag: 85:15 error "invalid operands to binary expression ('const char[2]' and 'int')"
//@ diag: 85:20 error "use of undeclared identifier 'val'; did you mean '::val'?"
//@ diag: 68:5 note "'::val' declared here"
//@ diag: 86:32 error "use of undeclared identifier 'val'; did you mean '::val'?"
//@ diag: 68:5 note "'::val' declared here"
//@ diag: 86:42 error "use of undeclared identifier 'val'; did you mean '::val'?"
//@ diag: 68:5 note "'::val' declared here"
//@ diag: 87:19 error "use of undeclared identifier 'val'; did you mean '::val'?"
//@ diag: 68:5 note "'::val' declared here"
//@ diag: 87:12 error "void function 's' should not return a value"
//@ diag: 88:18 error "use of undeclared identifier 'val'; did you mean '::val'?"
//@ diag: 68:5 note "'::val' declared here"
//@ diag: 88:18 error "cannot initialize return object of type 'int' with an lvalue of type 'int (int)'"
//@ diag: 89:30 error "use of undeclared identifier 'val'; did you mean '::val'?"
//@ diag: 68:5 note "'::val' declared here"
//@ diag: 90:21 error "use of undeclared identifier 'ufunc'; did you mean '::ufunc'?"
//@ diag: 71:39 note "'::ufunc' declared here"
//@ diag: 90:42 error "use of undeclared identifier 'redecl'; did you mean '::redecl'?"
//@ diag: 72:22 note "'::redecl' declared here"
//@ diag: 91:30 error "use of undeclared identifier 'val'"
//@ diag: 92:16 error "use of undeclared identifier 'val'"
//@ diag: 93:14 error "use of undeclared identifier 'val'"
//@ diag: 93:24 error "use of undeclared identifier 'val'"
//@ diag: 94:26 error "use of undeclared identifier 'val'"
//@ diag: 95:13 error "use of undeclared identifier 'val'"
//@ diag: 96:13 error "use of undeclared identifier 'val'"
//@ diag: 97:10 error "use of undeclared identifier 'val'"
//@ diag: 98:19 error "use of undeclared identifier 'val'"
int val(int);
int p3(int);
namespace N { int fn(); }
namespace U { int ufunc(); } using U::ufunc;
int redecl(int); int redecl(int);
void a() { val<int>; }
int (*b())(int) { return val<int>; }
int (*c)(int) = val<int>;
void d() { (void)val<int>; }
void e() { auto p = &val<int>; }
int f() { return (val<int>)(3); }
bool g() { return val<int> == nullptr; }
namespace N { void k() { auto q = fn<int>; } }
void m() { decltype(val<int>) *p = nullptr; }
void n() { if (val<int>) {} }
void o() { val<int> 2; }
void p() { val<int> + "x"*2; }
void q() { "y"*2 + val<int>; }
void r() { int (*a[2])(int) = {val<int>, val<int>}; }
void s() { return val<int>; }
int t() { return val<int>; }
template<class T> void u() { val<int>; }
void v() { auto w = ufunc<int>; auto x = redecl<int>; }
void x1() { int (*c)(char) = val<int>; }
void x2() { p3(val<int>); }
void x3() { (val<int>, val<int> + 1); }
void x4() { (void)sizeof(val<int>); }
void x5() { val<int>.x; }
void x6() { val<int> = 0; }
int x7 = val<int> + 1;
long (*x8)(int) = val<int>;
