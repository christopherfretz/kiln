// [dcl.spec.auto.general]/11, [temp.inst]/7: a specialization of a
// variable template declared with a deduced type whose initializer uses
// a specialization still being deduced, as clang 19 has it. The
// specialization the instantiation makes for a use of another
// specialization "requires an initializer"; one that names itself is a
// reference of type 'auto' (no diagnostic in the pattern), so an
// operator on it has "invalid operands" and the specialization keeps the
// type 'auto', which the variable it initializes cannot convert from
// (own-init-var-template-spec.cpp is clang 22's behaviour).
//@ std: c++26
//@ requires: clang<22
//@ expect: compile-fail
//@ strict-diags
//@ diag: 30:28 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 31:9 note "in instantiation of variable template specialization 'v' requested here"
//@ diag: 32:24 error "declaration of variable 'w' with deduced type 'auto' requires an initializer"
//@ diag: 32:35 note "in instantiation of variable template specialization 'w' requested here"
//@ diag: 34:24 error "declaration of variable 'e' with deduced type 'auto' requires an initializer"
//@ diag: 34:28 note "in instantiation of variable template specialization 'e' requested here"
//@ diag: 37:24 error "declaration of variable 'p' with deduced type 'auto' requires an initializer"
//@ diag: 37:32 note "in instantiation of variable template specialization 'p<T *>' requested here"
//@ diag: 39:52 error "declaration of variable 'm' with deduced type 'const auto' requires an initializer"
//@ diag: 39:56 note "in instantiation of static data member 'S::m' requested here"
//@ diag: 41:33 error "invalid operands to binary expression ('auto' and 'int')"
//@ diag: 42:10 note "in instantiation of variable template specialization 'f' requested here"
//@ diag: 44:5 error "cannot initialize a variable of type 'int' with an lvalue of type 'auto'"
//@ diag: 45:5 error "cannot initialize a variable of type 'int' with an lvalue of type 'auto'"
//@ diag: 46:11 error "cannot initialize a variable of type 'int' with an lvalue of type 'auto'"
//@ diag: 47:6 error "non-const lvalue reference to type 'int' cannot bind to a value of unrelated type 'auto'"
template<class T> auto v = T::x;
int a = v<int>;
template<class T> auto w = sizeof(w<int>);
int c = w<char>;
template<class T> auto e = e<int> + 1;
int e1 = e<int>;
template<class T> auto p = 0;
template<class T> auto p<T*> = p<int*>;
int p1 = p<int*>;
struct S { template<class T> static constexpr auto m = m<int>; };
int s = S::m<int>;
template<class T> auto f = f<T> + 1;
int f1 = f<int>;
template<class T> auto g = g<T>;
int g1 = g<int>;
int g2 = (g<int>);
int g3 = {g<int>};
int &g4 = g<int>;
