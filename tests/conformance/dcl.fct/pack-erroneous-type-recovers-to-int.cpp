// [dcl.fct]/23 (a declarator-id with an ellipsis declares a function
// parameter pack whose type must contain an unexpanded pack): when the
// pack's declared type is erroneous (an unknown type name, a bad member
// pointer, a reference to a reference) the parameter's type recovers to
// `int` as clang's does, so the pack check still runs on it and reports
// the recovered type. Valid packs (`T...`, `auto...`) are silent.
//@ expect: compile-fail
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 36:9 error "unknown type name 'Undecl'"
//@ diag: 36:15 error "type 'int' of function parameter pack does not contain any unexpanded parameter packs"
//@ diag: 37:30 error "unknown type name 'Undecl'"
//@ diag: 37:36 error "type 'int' of function parameter pack does not contain any unexpanded parameter packs"
//@ diag: 39:9 error "unknown type name 'Undecl'"
//@ diag: 39:17 error "type 'int *' of function parameter pack does not contain any unexpanded parameter packs"
//@ diag: 40:27 error "unknown type name 'Undecl'"
//@ diag: 40:33 error "type 'int' of function parameter pack does not contain any unexpanded parameter packs"
//@ diag: 42:10 error "type 'S' of function parameter pack does not contain any unexpanded parameter packs"
//@ diag: 44:10 error "unknown type name 'Undecl'"
//@ diag: 44:18 error "type 'int &&' of function parameter pack does not contain any unexpanded parameter packs"
//@ diag: 45:18 error "'x' declared as array of references of type 'int &'"
//@ diag: 45:14 error "type 'int' of function parameter pack does not contain any unexpanded parameter packs"
//@ diag: 46:13 error "type 'void' of function parameter pack does not contain any unexpanded parameter packs"
//@ diag: 46:17 error "argument may not have 'void' type"
//@ diag: 47:13 error "use of undeclared identifier 'U'"
//@ diag: 47:17 error "type 'int' of function parameter pack does not contain any unexpanded parameter packs"
//@ diag: 48:18 error "use of undeclared identifier 'undecl'"
//@ diag: 48:25 error "type 'int' of function parameter pack does not contain any unexpanded parameter packs"
//@ diag: 49:15 error "'x' declared as a reference to a reference"
//@ diag: 49:16 error "type 'int &' of function parameter pack does not contain any unexpanded parameter packs"
//@ diag: 50:18 error "use of undeclared identifier 'X'"
//@ diag: 50:22 error "type 'int' of function parameter pack does not contain any unexpanded parameter packs"
//@ diag: 52:9 error "use of class template 'Q' requires template arguments; argument deduction not allowed in function prototype"
//@ diag: 51:26 note "template is declared here"
//@ diag: 52:10 error "type 'int' of function parameter pack does not contain any unexpanded parameter packs"
void f1(Undecl... x);
template<class... T> void f2(Undecl... x);
template<class... T> void f3(typename T::type... x);
void f5(Undecl *... x);
template<class T> void f6(Undecl... x, T t);
struct S;
void f7(S... s);
void f8(auto... a);
void f10(Undecl&&... x);
void g1(int &...x[3]);
void g3(void... v);
void g4(int U::*... m);
void g5(decltype(undecl)... x);
void g6(int & &... x);
void g7(typename X::y... x);
template<class T> struct Q;
void g9(Q... q);
