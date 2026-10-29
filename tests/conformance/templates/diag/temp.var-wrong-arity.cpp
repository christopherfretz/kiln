// [temp.arg.general]/1, [temp.var]: variable template arguments are
// checked against the parameter list.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:9 error "too many template arguments for variable template 'v'"
//@ diag: 11:34 note "template is declared here"
//@ diag: 14:11 error "template argument for non-type template parameter must be an expression"
//@ diag: 12:15 note "template parameter is declared here"
//@ diag: 15:11 error "template argument for template type parameter must be a type"
//@ diag: 11:17 note "template parameter is declared here"
template <class T> constexpr int v = 0;
template <int N> constexpr int w = N;
int a = v<int, int>;
int b = w<int>;
int c = v<1>;
