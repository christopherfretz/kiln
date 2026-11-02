// [temp.arg.general]/2 (arity), [basic.lookup.general]/1: the note on a
// wrong number of template arguments names the declaration name lookup
// finds, the latest one: a redeclaration after the definition, not the
// definition (as clang).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 23:1 error "too few template arguments for class template 'S'"
//@ diag: 16:36 note "template is declared here"
//@ diag: 24:1 error "too few template arguments for class template 'R'"
//@ diag: 19:34 note "template is declared here"
//@ diag: 25:1 error "too many template arguments for class template 'D'"
//@ diag: 20:56 note "template is declared here"
//@ diag: 26:9 error "too many template arguments for variable template 'v'"
//@ diag: 22:37 note "template is declared here"
template <class T, class U> struct S {};
template <class T, class U> struct S;
template <class T, int N> struct R;
template <class T, int N> struct R {};
template <class T, int N> struct R;
template <class T> struct D; template <class T> struct D {};
template <class T> constexpr int v = 1;
template <class T> extern const int v;
S<int> s;
R<int> r;
D<int, int> *d;
int x = v<int, int>;
