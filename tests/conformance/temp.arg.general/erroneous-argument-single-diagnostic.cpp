// [temp.arg.general]/1, [temp.arg.type]: an erroneous template argument
// (an undeclared name) is diagnosed once where it is formed; the
// template-id is then invalid without a second "template argument for
// template type parameter must be a type" (clang's RecoveryExpr), also
// through an alias template.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:16 error "use of undeclared identifier 'E7'"
//@ diag: 14:3 error "use of undeclared identifier 'undeclared_name'"
template <class T> struct ut { typedef T type; };
template <class T> using ut_t = typename ut<T>::type;
enum E7 : ut_t<E7> {};
template <class T> struct P {};
P<undeclared_name> p;
