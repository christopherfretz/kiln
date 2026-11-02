// [dcl.init.list]/7: a compound literal of type bool is list-initialized,
// so int, floating, pointer and out-of-range constant sources are narrowing.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 18:31 error "non-constant-expression cannot be narrowed from type 'int' to 'bool' in initializer list"
//@ diag: 18:31 note "insert an explicit cast to silence this issue"
//@ diag: 19:26 error "constant expression evaluates to 2 which cannot be narrowed to type 'bool'"
//@ diag: 19:26 note "insert an explicit cast to silence this issue"
//@ diag: 20:34 error "type 'double' cannot be narrowed to 'bool' in initializer list"
//@ diag: 20:34 note "insert an explicit cast to silence this issue"
//@ diag: 23:32 error "type 'int *' cannot be narrowed to 'bool' in initializer list"
//@ diag: 23:32 note "insert an explicit cast to silence this issue"
//@ diag: 25:23 error "constant expression evaluates to 5 which cannot be narrowed to type 'bool'"
//@ diag: 25:23 note "insert an explicit cast to silence this issue"
//@ diag: 22:48 error "non-constant-expression cannot be narrowed from type 'int' to 'bool' in initializer list"
//@ diag: 24:9 note "in instantiation of function template specialization 'g<int>' requested here"
//@ diag: 22:48 note "insert an explicit cast to silence this issue"
bool c(int i) { return (bool){i}; }
bool d() { return (bool){2}; }
bool e(double x) { return (bool){x}; }
bool f() { return (bool){1} && (bool){0} && (bool){true}; }
template <class T> bool g(T t) { return (bool){t}; }
bool h(int *p) { return (bool){p}; }
int u = g(3);
const bool k = (bool){5};
