// A statement expression in a default member initializer or a member
// function's default argument of a class that is not local to a function
// (also a class template's) is not in a function body (clang's
// err_stmtexpr_file_scope, at the '('); in a local class it is, and its
// locals are the enclosing function's (class.local/stmt-expr-local-in-*,
// WP-2474).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:21 error "statement expression not allowed at file scope"
//@ diag: 14:27 error "statement expression not allowed at file scope"
//@ diag: 15:39 error "statement expression not allowed at file scope"
//@ diag: 17:32 error "statement expression not allowed at file scope"
struct N { int fl = ({ int tf = 2; tf; }); };
struct M { void m(int a = ({ int tf = 5; tf; })); };
template <class T> struct TT { T fl = ({ T tf = 2; tf; }); };
TT<int> tti;
struct O { struct I { int fl = ({ int tf = 2; tf; }); }; };
void ok() { struct L { int fl = ({ int tf = 2; tf; }); }; }
