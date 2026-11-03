// [class.copy.assign]/12, [temp.inst]/11: a defaulted copy or move
// assignment operator is implicitly defined at the end of its declaration
// (clang `DefineImplicitCopyAssignment` uses `getEndLoc()`), so an
// instantiation of a member's `operator=` it calls is "requested" there: at
// the last character of `default` for one defaulted on its first
// declaration (a class template member included), at the closing `)` of the
// declarator for one defaulted out of line, and at the class for an
// implicitly declared one.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 23:28 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 26:34 note "in instantiation of member function 'M<int>::operator=' requested here"
//@ diag: 23:28 error "type 'long' cannot be used prior to '::' because it has no members"
//@ diag: 34:25 note "in instantiation of member function 'M<long>::operator=' requested here"
//@ diag: 23:28 error "type 'char' cannot be used prior to '::' because it has no members"
//@ diag: 40:31 note "in instantiation of member function 'M<char>::operator=' requested here"
//@ diag: 41:36 note "in instantiation of member function 'E<char>::operator=' requested here"
//@ diag: 23:28 error "type 'short' cannot be used prior to '::' because it has no members"
//@ diag: 43:38 note "in instantiation of member function 'M<short>::operator=' requested here"
//@ diag: 23:28 error "type 'unsigned int' cannot be used prior to '::' because it has no members"
//@ diag: 47:8 note "in instantiation of member function 'M<unsigned int>::operator=' requested here"
template<class T> struct M {
  M& operator=(const M&) { T::fail(); return *this; }
};
struct C {
  C& operator=(const C&) = default  ;
  M<int> m;
};
void f(C& a, C& b) { a = b; }
struct D {
  D& operator=(const D&);
  M<long> m;
};
D& D::operator=(const D&) = default;
template<class T> struct E {
  E& operator=(const E&);
  M<T> m;
};
template<class T>
E<T>& E<T>::operator=(const E&) = default;
void g(E<char>& a, E<char>& b) { a = b; }
template<class T> struct F {
  F& operator=(F&&) noexcept = default;
  M<T> m;
};
void h(F<short>& a, F<short>& b) { a = static_cast<F<short>&&>(b); }
struct G { M<unsigned> m; };
void i(G& a, G& b) { a = b; }
