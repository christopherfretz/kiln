// [over.match.best], clang 21+: the function templates are deduced, and a
// hard error in a signature is reported, when no non-template candidate is a
// unique perfect match: a reference bound with added cv-qualifiers, a
// qualification or function pointer conversion, a braced-init-list argument,
// a static member function (its object argument is never a perfect match), a
// prvalue object for a member function without ref-qualifier (an lvalue
// reference bound to an rvalue), two perfect candidates neither more
// constrained (an ambiguity resolved, or reported, with every candidate). A
// deleted perfect match is the result, its notes listing the templates,
// deduced for them (perfect-match-reversed-member.cpp: a member operator==).
//@ expect: compile-fail
//@ requires: clang>=22
//@ strict-diags
//@ diag: 54:17 error "static assertion failed due to requirement 'sizeof(E1) == 0': instantiated"
//@ diag: 61:29 note "in instantiation of template class 'bad<E1>' requested here"
//@ diag: 83:3 note "while substituting deduced template arguments into function template 'q1' [with T = E1]"
//@ diag: 54:27 note "expression evaluates to '4 == 0'"
//@ diag: 54:17 error "static assertion failed due to requirement 'sizeof(E2 *) == 0': instantiated"
//@ diag: 63:29 note "in instantiation of template class 'bad<E2 *>' requested here"
//@ diag: 84:3 note "while substituting deduced template arguments into function template 'q2' [with T = E2 *]"
//@ diag: 54:27 note "expression evaluates to '8 == 0'"
//@ diag: 54:17 error "static assertion failed due to requirement 'sizeof(void (*)() noexcept) == 0': instantiated"
//@ diag: 66:29 note "in instantiation of template class 'bad<void (*)() noexcept>' requested here"
//@ diag: 85:3 note "while substituting deduced template arguments into function template 'q3' [with T = void (*)() noexcept]"
//@ diag: 54:27 note "expression evaluates to '8 == 0'"
//@ diag: 54:17 error "static assertion failed due to requirement 'sizeof(E4) == 0': instantiated"
//@ diag: 68:29 note "in instantiation of template class 'bad<E4>' requested here"
//@ diag: 86:3 note "while substituting deduced template arguments into function template 'q4' [with T = E4]"
//@ diag: 54:27 note "expression evaluates to '4 == 0'"
//@ diag: 86:10 warning "braces around scalar initializer"
//@ diag: 54:17 error "static assertion failed due to requirement 'sizeof(E5) == 0': instantiated"
//@ diag: 71:38 note "in instantiation of template class 'bad<E5>' requested here"
//@ diag: 87:3 note "while substituting deduced template arguments into function template 'st' [with T = E5]"
//@ diag: 54:27 note "expression evaluates to '4 == 0'"
//@ diag: 54:17 error "static assertion failed due to requirement 'sizeof(E6) == 0': instantiated"
//@ diag: 73:31 note "in instantiation of template class 'bad<E6>' requested here"
//@ diag: 88:7 note "while substituting deduced template arguments into function template 'm' [with T = E6]"
//@ diag: 54:27 note "expression evaluates to '4 == 0'"
//@ diag: 54:17 error "static assertion failed due to requirement 'sizeof(E7) == 0': instantiated"
//@ diag: 77:29 note "in instantiation of template class 'bad<E7>' requested here"
//@ diag: 89:3 note "while substituting deduced template arguments into function template 'amb' [with T = E7]"
//@ diag: 54:27 note "expression evaluates to '4 == 0'"
//@ diag: 89:3 error "call to 'amb' is ambiguous"
//@ diag: 75:5 note "candidate function"
//@ diag: 76:5 note "candidate function"
//@ diag: 54:17 error "static assertion failed due to requirement 'sizeof(E8) == 0': instantiated"
//@ diag: 79:29 note "in instantiation of template class 'bad<E8>' requested here"
//@ diag: 90:3 note "while substituting deduced template arguments into function template 'del' [with T = E8]"
//@ diag: 54:27 note "expression evaluates to '4 == 0'"
//@ diag: 90:3 error "call to deleted function 'del'"
//@ diag: 78:5 note "candidate function has been explicitly deleted"
//@ diag: 79:42 note "candidate template ignored: substitution failure [with T = E8]"
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
}
