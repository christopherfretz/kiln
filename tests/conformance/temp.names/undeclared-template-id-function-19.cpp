// [temp.names]/3: undeclared-template-id-function-22.cpp's forms under
// clang 19, which reports the template-id's name as undeclared (a delayed
// typo whose correction to the qualified name fails in these
// expressions).
//@ requires: clang<22
//@ expect: compile-fail
//@ strict-diags
//@ args: -ferror-limit=0
//@ diag: 25:56 error "use of undeclared identifier 'fn'"
//@ diag: 32:18 error "use of undeclared identifier 'val'"
//@ diag: 33:19 error "use of undeclared identifier 'val'"
//@ diag: 34:18 error "use of undeclared identifier 'redecl'"
//@ diag: 35:18 error "use of undeclared identifier 'ovset'"
//@ diag: 36:18 error "use of undeclared identifier 'ufunc'"
//@ diag: 37:18 error "use of undeclared identifier 'mixed'"
//@ diag: 38:18 error "use of undeclared identifier 'amb'"
//@ diag: 39:36 error "use of undeclared identifier 'abcdef'"
//@ diag: 40:20 error "'val' does not name a template but is followed by template arguments"
//@ diag: 21:5 note "non-template declaration found by name lookup"
//@ diag: 41:18 error "use of undeclared identifier 'val'"
int val(int);
int redecl(int);
int redecl(int);
int ovset(int); int ovset(char);
namespace N { int fn(); namespace M { int g() { return fn<int> + 1; } } }
namespace U { int ufunc(); } using U::ufunc;
int mixed(char);
namespace X { int mixed(int); }
using X::mixed;
namespace Y { int amb(); } namespace Z { int amb(); } using namespace Y; using namespace Z;
int abcdef(int);
int a() { return val<int> + 1; }
int b() { return (val<int> * 2); }
int c() { return redecl<int> + 1; }
int d() { return ovset<int> + 1; }
int e() { return ufunc<int> + 1; }
int f() { return mixed<int> + 1; }
int g() { return amb<int> + 1; }
template<class T> int h() { return abcdef<T> + 1; }
int i() { return ::val<int> + 1; }
int j() { return val<int>(3); }
