// [temp.arg.general]/2, [temp.arg.type]/1, [temp.arg.nontype]/1,
// [temp.arg.template]/1: each argument must be of the parameter's kind.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:7 error "template argument for template type parameter must be a type"
//@ diag: 10:17 note "template parameter is declared here"
//@ diag: 14:6 error "template argument for non-type template parameter must be an expression"
//@ diag: 11:15 note "template parameter is declared here"
//@ diag: 15:7 error "template argument for template template parameter must be a class template or type alias template"
template <class T> struct TypeP {};
template <int N> struct ValP {};
template <template <class> class F> struct TmplP {};
TypeP<3> a;
ValP<int> b;
TmplP<int> c;
