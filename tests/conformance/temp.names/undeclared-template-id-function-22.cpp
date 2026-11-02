// [temp.names]/3: a name ordinary lookup finds only functions for, followed
// by '<', is a template-name; not called, the template-id names nothing.
// clang 22 typo-corrects the name to itself qualified by its namespace
// ("did you mean '::val'?", the note at the first function declared, or
// at the using-declaration naming it) and goes on with that function's
// overload set; functions from several namespaces, or found both directly
// and through a using-declaration, get no correction. A qualified name
// followed by '<' is diagnosed as not naming a template.
//@ requires: clang>=22
//@ expect: compile-fail
//@ strict-diags
//@ args: -ferror-limit=0
//@ diag: 48:56 error "use of undeclared identifier 'fn'; did you mean '::N::fn'?"
//@ diag: 48:19 note "'::N::fn' declared here"
//@ diag: 48:64 error "arithmetic on a pointer to the function type 'int ()'"
//@ diag: 55:18 error "use of undeclared identifier 'val'; did you mean '::val'?"
//@ diag: 44:5 note "'::val' declared here"
//@ diag: 55:27 error "arithmetic on a pointer to the function type 'int (int)'"
//@ diag: 56:19 error "use of undeclared identifier 'val'; did you mean '::val'?"
//@ diag: 44:5 note "'::val' declared here"
//@ diag: 56:28 error "invalid operands to binary expression ('int (int)' and 'int')"
//@ diag: 57:18 error "use of undeclared identifier 'redecl'; did you mean '::redecl'?"
//@ diag: 46:5 note "'::redecl' declared here"
//@ diag: 57:30 error "arithmetic on a pointer to the function type 'int (int)'"
//@ diag: 58:18 error "use of undeclared identifier 'ovset'; did you mean '::ovset'?"
//@ diag: 47:5 note "'::ovset' declared here"
//@ diag: 58:29 error "arithmetic on a pointer to the function type 'int (int)'"
//@ diag: 59:18 error "use of undeclared identifier 'ufunc'; did you mean '::ufunc'?"
//@ diag: 49:39 note "'::ufunc' declared here"
//@ diag: 59:29 error "arithmetic on a pointer to the function type 'int ()'"
//@ diag: 60:18 error "use of undeclared identifier 'mixed'"
//@ diag: 61:18 error "use of undeclared identifier 'amb'"
//@ diag: 62:36 error "use of undeclared identifier 'abcdef'; did you mean '::abcdef'?"
//@ diag: 54:5 note "'::abcdef' declared here"
//@ diag: 63:20 error "'val' does not name a template but is followed by template arguments"
//@ diag: 44:5 note "non-template declaration found by name lookup"
//@ diag: 64:18 error "use of undeclared identifier 'val'"
//@ diag: 65:19 error "use of undeclared identifier 'abcdef'; did you mean '::abcdef'?"
//@ diag: 54:5 note "'::abcdef' declared here"
//@ diag: 65:31 error "expected ')'"
//@ diag: 65:18 note "to match this '('"
//@ diag: 65:19 error "address of overloaded function 'abcdef' does not match required type 'int'"
//@ diag: 54:5 note "candidate function"
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
int k() { return (abcdef<int> 5); }
