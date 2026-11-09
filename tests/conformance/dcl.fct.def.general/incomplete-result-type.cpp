// [dcl.fct.def.general]/2: the return type of a function definition shall
// not be an incomplete class type (clang's err_func_def_incomplete_result,
// checked before the parameters). A dependent, void, reference or deleted
// return type, a declaration, and a class completed later are fine.
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 16:13 error "incomplete result type 'X' in function definition"
//@ diag: 16:8 note "forward declaration of 'X'"
//@ diag: 17:6 error "incomplete result type 'struct Y' in function definition"
//@ diag: 17:20 note "forward declaration of 'Y'"
//@ diag: 18:27 error "incomplete result type 'X' in function definition"
//@ diag: 16:8 note "forward declaration of 'X'"
//@ diag: 19:21 error "incomplete result type 'X' in function definition"
//@ diag: 16:8 note "forward declaration of 'X'"
struct X; X f() {}
auto g() -> struct Y {}
struct M { X m(); }; X M::m() {}
template<class T> X h(T) {}
X &ok1() { static X *p; return *p; }
X ok2() = delete;
X ok3();
template<class T> T ok4() { return T(); }
struct L; L later();
struct L {}; L later() { return {}; }
