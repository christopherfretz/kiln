// [temp.names]/1, [temp.var]: a variable template must be named with a
// template argument list.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 10:9 error "use of variable template 'size' requires template arguments"
//@ diag: 9:34 note "template is declared here"
//@ diag: 11:18 error "use of variable template 'size' requires template arguments"
//@ diag: 9:34 note "template is declared here"
template <class T> constexpr int size = sizeof(T);
int a = size;
int f() { return size + 1; }
