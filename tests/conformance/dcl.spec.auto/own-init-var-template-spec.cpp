// [dcl.spec.auto.general]/11, [temp.inst]/7: a specialization of a
// variable template declared with a deduced type whose initializer uses
// a specialization still being deduced. Clang 22: the specialization the
// instantiation makes for that use "requires an initializer" (its "in
// instantiation" note names the pattern), and every later use of a
// specialization left undeduced, also one whose instantiation failed
// (`v<int>`), is "the type of variable template specialization 'v<int>'
// declared with deduced type 'auto' depends on itself". A pattern that
// names itself is diagnosed once, in the pattern
// (own-init-var-template.cpp).
// own-init-var-template-spec-19.cpp is clang 19's behaviour.
//@ std: c++26
//@ requires: clang>=22
//@ expect: compile-fail
//@ strict-diags
//@ diag: 35:28 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 36:9 note "in instantiation of variable template specialization 'v' requested here"
//@ diag: 37:9 error "the type of variable template specialization 'v<int>' declared with deduced type 'auto' depends on itself"
//@ diag: 38:24 error "declaration of variable 'w' with deduced type 'auto' requires an initializer"
//@ diag: 38:35 note "in instantiation of variable template specialization 'w' requested here"
//@ diag: 40:9 error "the type of variable template specialization 'w<int>' declared with deduced type 'auto' depends on itself"
//@ diag: 41:24 error "declaration of variable 'e' with deduced type 'auto' requires an initializer"
//@ diag: 41:28 note "in instantiation of variable template specialization 'e' requested here"
//@ diag: 42:10 error "the type of variable template specialization 'e<int>' declared with deduced type 'auto' depends on itself"
//@ diag: 43:10 error "the type of variable template specialization 'e<int>' declared with deduced type 'auto' depends on itself"
//@ diag: 45:24 error "declaration of variable 'p' with deduced type 'auto' requires an initializer"
//@ diag: 45:32 note "in instantiation of variable template specialization 'p<T *>' requested here"
//@ diag: 46:10 error "the type of variable template specialization 'p<int *>' declared with deduced type 'auto' depends on itself"
//@ diag: 47:52 error "declaration of variable 'm' with deduced type 'const auto' requires an initializer"
//@ diag: 47:56 note "in instantiation of static data member 'S::m' requested here"
//@ diag: 48:12 error "the type of variable template specialization 'm<int>' declared with deduced type 'const auto' depends on itself"
//@ diag: 49:74 error "the type of variable template specialization 'q<int>' declared with deduced type 'const auto' depends on itself"
//@ diag: 49:74 note "in instantiation of static data member 'Y<char>::q<int>' requested here"
//@ diag: 50:18 note "in instantiation of static data member 'Y<char>::q<char>' requested here"
template<class T> auto v = T::x;
int a = v<int>;
int b = v<int>;
template<class T> auto w = sizeof(w<int>);
int c = w<char>;
int d = w<int>;
template<class T> auto e = e<int> + 1;
int e1 = e<int>;
int e2 = e<int>;
template<class T> auto p = 0;
template<class T> auto p<T*> = p<int*>;
int p1 = p<int*>;
struct S { template<class T> static constexpr auto m = m<int>; };
int s = S::m<int>;
template<class T> struct Y { template<class U> static constexpr auto q = q<int>; };
int y = Y<char>::q<char>;
