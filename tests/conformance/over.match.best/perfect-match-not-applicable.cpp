// [over.match.best], clang 21+: the function templates are deduced, and a
// hard error in a signature is reported, when no non-template candidate is a
// unique perfect match: a reference bound with added cv-qualifiers, a
// qualification or function pointer conversion, a braced-init-list argument,
// a static member function (its object argument is never a perfect match), a
// prvalue object for a member function without ref-qualifier (an lvalue
// reference bound to an rvalue), two perfect candidates neither more
// constrained (an ambiguity resolved, or reported, with every candidate), a
// member `operator==` and its own reversed form. A deleted perfect match is
// the result, its notes listing the templates, deduced for them.
//@ expect: compile-fail
//@ requires: clang>=22
//@ strict-diags
//@ diag: 58:17 error "static assertion failed due to requirement 'sizeof(E1) == 0': instantiated"
//@ diag: 65:29 note "in instantiation of template class 'bad<E1>' requested here"
//@ diag: 91:3 note "while substituting deduced template arguments into function template 'q1' [with T = E1]"
//@ diag: 58:27 note "expression evaluates to '4 == 0'"
//@ diag: 58:17 error "static assertion failed due to requirement 'sizeof(E2 *) == 0': instantiated"
//@ diag: 67:29 note "in instantiation of template class 'bad<E2 *>' requested here"
//@ diag: 92:3 note "while substituting deduced template arguments into function template 'q2' [with T = E2 *]"
//@ diag: 58:27 note "expression evaluates to '8 == 0'"
//@ diag: 58:17 error "static assertion failed due to requirement 'sizeof(void (*)() noexcept) == 0': instantiated"
//@ diag: 70:29 note "in instantiation of template class 'bad<void (*)() noexcept>' requested here"
//@ diag: 93:3 note "while substituting deduced template arguments into function template 'q3' [with T = void (*)() noexcept]"
//@ diag: 58:27 note "expression evaluates to '8 == 0'"
//@ diag: 58:17 error "static assertion failed due to requirement 'sizeof(E4) == 0': instantiated"
//@ diag: 72:29 note "in instantiation of template class 'bad<E4>' requested here"
//@ diag: 94:3 note "while substituting deduced template arguments into function template 'q4' [with T = E4]"
//@ diag: 58:27 note "expression evaluates to '4 == 0'"
//@ diag: 94:10 warning "braces around scalar initializer"
//@ diag: 58:17 error "static assertion failed due to requirement 'sizeof(E5) == 0': instantiated"
//@ diag: 75:38 note "in instantiation of template class 'bad<E5>' requested here"
//@ diag: 95:3 note "while substituting deduced template arguments into function template 'st' [with T = E5]"
//@ diag: 58:27 note "expression evaluates to '4 == 0'"
//@ diag: 58:17 error "static assertion failed due to requirement 'sizeof(E6) == 0': instantiated"
//@ diag: 77:31 note "in instantiation of template class 'bad<E6>' requested here"
//@ diag: 96:7 note "while substituting deduced template arguments into function template 'm' [with T = E6]"
//@ diag: 58:27 note "expression evaluates to '4 == 0'"
//@ diag: 58:17 error "static assertion failed due to requirement 'sizeof(E7) == 0': instantiated"
//@ diag: 81:29 note "in instantiation of template class 'bad<E7>' requested here"
//@ diag: 97:3 note "while substituting deduced template arguments into function template 'amb' [with T = E7]"
//@ diag: 58:27 note "expression evaluates to '4 == 0'"
//@ diag: 97:3 error "call to 'amb' is ambiguous"
//@ diag: 79:5 note "candidate function"
//@ diag: 80:5 note "candidate function"
//@ diag: 58:17 error "static assertion failed due to requirement 'sizeof(E8) == 0': instantiated"
//@ diag: 83:29 note "in instantiation of template class 'bad<E8>' requested here"
//@ diag: 98:3 note "while substituting deduced template arguments into function template 'del' [with T = E8]"
//@ diag: 58:27 note "expression evaluates to '4 == 0'"
//@ diag: 98:3 error "call to deleted function 'del'"
//@ diag: 82:5 note "candidate function has been explicitly deleted"
//@ diag: 83:42 note "candidate template ignored: substitution failure [with T = E8]"
//@ diag: 58:17 error "static assertion failed due to requirement 'sizeof(Q) == 0': instantiated"
//@ diag: 86:31 note "in instantiation of template class 'bad<Q>' requested here"
//@ diag: 100:12 note "while substituting deduced template arguments into function template 'operator==' [with T = Q]"
//@ diag: 58:27 note "expression evaluates to '1 == 0'"
template <class T> struct bad {
  static_assert(sizeof(T) == 0, "instantiated");
  typedef int type;
};
enum E1 { A1 }; enum E2 { A2 }; enum E3 { A3 }; enum E4 { A4 }; enum E5 { A5 };
enum E6 { A6 }; enum E7 { A7 }; enum E8 { A8 }; enum E9 { A9 };

int q1(const E1 &);
template <class T> typename bad<T>::type q1(T);
int q2(const E2 *);
template <class T> typename bad<T>::type q2(T);
void fn() noexcept;
int q3(void (*)());
template <class T> typename bad<T>::type q3(T, E3 = A3);
int q4(E4, int);
template <class T> typename bad<T>::type q4(T, int);
struct S {
  static int st(E5);
  template <class T> static typename bad<T>::type st(T);
  int m(E6);
  template <class T> typename bad<T>::type m(T);
};
int amb(E7);
int amb(E7 &);
template <class T> typename bad<T>::type amb(T);
int del(E8) = delete;
template <class T> typename bad<T>::type del(T);
struct Q {
  bool operator==(const Q &) const;
  template <class T> typename bad<T>::type operator==(const T &) const;
};

void use() {
  E1 e1 = A1; E2 e2 = A2; E4 e4 = A4; E5 e5 = A5; E6 e6 = A6; E7 e7 = A7; E8 e8 = A8;
  q1(e1);
  q2(&e2);
  q3(fn);
  q4(e4, {1});
  S::st(e5);
  S().m(e6);
  amb(e7);
  del(e8);
  const Q q;
  (void)(q == q);
}
