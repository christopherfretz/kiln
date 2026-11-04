// [over.match.best], [temp.deduct]/8: before clang 21 every function
// template candidate is deduced, beside a non-template candidate that is a
// perfect match too, so substituting a signature that is a hard error
// (`bad<T>::type`, as boost::is_unsigned<enum> for the reference workload's
// ranking-traits header) is reported even though the
// non-template would be chosen (run-perfect-match-skips-templates.cpp is the
// clang 21+ behaviour).
//@ expect: compile-fail
//@ requires: clang<22
//@ args: -fclang-compat=19
//@ strict-diags
//@ diag: 21:17 error "static assertion failed due to requirement 'sizeof(YN) == 0': instantiated"
//@ diag: 27:29 note "in instantiation of template class 'bad<YN>' requested here"
//@ diag: 35:10 note "while substituting deduced template arguments into function template 'rank' [with T = YN]"
//@ diag: 21:27 note "expression evaluates to '4 == 0'"
//@ diag: 21:17 error "static assertion failed due to requirement 'sizeof(E) == 0': instantiated"
//@ diag: 30:31 note "in instantiation of template class 'bad<E>' requested here"
//@ diag: 35:22 note "while substituting deduced template arguments into function template 'f' [with T = E]"
//@ diag: 21:27 note "expression evaluates to '4 == 0'"
template <class T> struct bad {
  static_assert(sizeof(T) == 0, "instantiated");
  typedef int type;
};
enum YN { NO, YES };
enum E { A };
int rank(YN) { return 4; }
template <class T> typename bad<T>::type rank(T) { return 0; }
struct S {
  int f(E) { return 4; }
  template <class T> typename bad<T>::type f(T) { return 0; }
};
int main() {
  YN v = YES;
  S s;
  return rank(v) + s.f(A);
}
