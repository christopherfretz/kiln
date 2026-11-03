// [basic.scope.scope]/4, [temp.pre]/7: a class template's name cannot be
// redeclared as a non-template class, a variable, a function or a
// variable, function or alias template, in either order; `template<>`
// before a non-template class is not a specialization.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 35:8 error "redefinition of 'X1' as different kind of symbol"
//@ diag: 34:27 note "previous definition is here"
//@ diag: 37:8 error "redefinition of 'X2' as different kind of symbol"
//@ diag: 36:27 note "previous definition is here"
//@ diag: 39:27 error "redefinition of 'Y1' as different kind of symbol"
//@ diag: 38:8 note "previous definition is here"
//@ diag: 41:27 error "redefinition of 'Y2' as different kind of symbol"
//@ diag: 40:8 note "previous definition is here"
//@ diag: 43:27 error "redefinition of 'V1' as different kind of symbol"
//@ diag: 42:5 note "previous definition is here"
//@ diag: 45:5 error "redefinition of 'V2' as different kind of symbol"
//@ diag: 44:27 note "previous definition is here"
//@ diag: 47:27 error "redefinition of 'F1' as different kind of symbol"
//@ diag: 46:6 note "previous definition is here"
//@ diag: 49:6 error "redefinition of 'F2' as different kind of symbol"
//@ diag: 48:27 note "previous definition is here"
//@ diag: 51:27 error "redefinition of 'G1' as different kind of symbol"
//@ diag: 50:24 note "previous definition is here"
//@ diag: 53:24 error "redefinition of 'G2' as different kind of symbol"
//@ diag: 52:27 note "previous definition is here"
//@ diag: 55:27 error "redefinition of 'H1' as different kind of symbol"
//@ diag: 54:25 note "previous definition is here"
//@ diag: 57:1 error "extraneous 'template<>' in declaration of struct 'NT'"
//@ diag: 57:20 error "specialization of member 'NT' does not specialize an instantiated member"
//@ diag: 56:8 note "attempt to specialize declaration here"
//@ diag: 59:27 error "redefinition of 'AT' as different kind of symbol"
//@ diag: 58:20 note "previous definition is here"
template <class T> struct X1;
struct X1;
template <class T> struct X2 {};
struct X2 {};
struct Y1;
template <class T> struct Y1;
struct Y2 {};
template <class T> struct Y2 {};
int V1;
template <class T> struct V1;
template <class T> struct V2;
int V2;
void F1();
template <class T> struct F1;
template <class T> struct F2;
void F2();
template <class T> int G1;
template <class T> struct G1;
template <class T> struct G2;
template <class T> int G2;
template <class T> void H1();
template <class T> struct H1;
struct NT {};
template <> struct NT;
template <class T> using AT = int;
template <class T> struct AT;
