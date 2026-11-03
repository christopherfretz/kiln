// [temp.param]/1: a template template parameter's template-head has a
// non-empty parameter list (`template<>` is not one).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 11:37 error "template template parameter must have its own template parameters"
//@ diag: 12:28 error "template template parameter must have its own template parameters"
//@ diag: 13:27 error "template template parameter must have its own template parameters"
//@ diag: 13:36 error "redefinition of 'A2'"
//@ diag: 12:38 note "previous definition is here"
//@ diag: 14:30 error "template template parameter must have its own template parameters"
template <class T, template<> class U> struct A1 {};
template <template<> class U> struct A2 {};
template <template<> class> struct A2 {};
template <template<> class...U> struct A4 {};
