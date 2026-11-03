// [conv.bool]/1, [temp.inst]: the -Wint-in-bool-context and
// -Wtautological-constant-compare checks of an integer converted to
// `bool` run in the template definition for a non-dependent
// full-expression and again in each instantiation, with the
// instantiation note (clang re-checks every rebuilt operator and every
// full-expression); a value-dependent full-expression (`N << 1`,
// `0 << N`, `(x << 1) && N`) is checked only in each instantiation
// (clang's `CheckImplicitConversions` skips it). A substituted constant
// is not an integer literal (`a ? N : 5`). Unsigned operands (`f(2u)`)
// are not warned. clang 22 wording (clang 19: int-in-bool-per-instantiation-19.cpp) (WP-419).
//@ expect: compile-ok
//@ strict-diags
//@ requires: clang>=22
//@ args: -Wint-in-bool-context
//@ diag: 60:9 warning "converting the result of '<<' to a boolean; did you mean to compare with '0'?"
//@ diag: 66:45 warning "converting the enum constant to a boolean"
//@ diag: 69:10 warning "converting the result of '<<' to a boolean; did you mean to compare with '0'?"
//@ diag: 69:26 warning "converting the enum constant to a boolean"
//@ diag: 70:15 warning "converting the result of '<<' to a boolean; did you mean to compare with '0'?"
//@ diag: 74:14 warning "converting the result of '<<' to a boolean; did you mean to compare with '0'?"
//@ diag: 75:16 warning "converting the result of '<<' to a boolean; did you mean to compare with '0'?"
//@ diag: 56:9 warning "converting the result of '<<' to a boolean; did you mean to compare with '0'?"
//@ diag: 79:10 note "in instantiation of function template specialization 'f<int>' requested here"
//@ diag: 56:9 warning "converting the result of '<<' to a boolean; did you mean to compare with '0'?"
//@ diag: 79:27 note "in instantiation of function template specialization 'f<long>' requested here"
//@ diag: 60:9 warning "converting the result of '<<' to a boolean; did you mean to compare with '0'?"
//@ diag: 79:36 note "in instantiation of function template specialization 'g<int>' requested here"
//@ diag: 60:9 warning "converting the result of '<<' to a boolean; did you mean to compare with '0'?"
//@ diag: 79:49 note "in instantiation of function template specialization 'g<char>' requested here"
//@ diag: 63:38 warning "converting the result of '<<' to a boolean; did you mean to compare with '0'?"
//@ diag: 79:63 note "in instantiation of function template specialization 'h<1>' requested here"
//@ diag: 65:43 warning "converting the enum constant to a boolean"
//@ diag: 80:21 note "in instantiation of function template specialization 'm<int>' requested here"
//@ diag: 66:45 warning "converting the enum constant to a boolean"
//@ diag: 80:29 note "in instantiation of function template specialization 'p<int>' requested here"
//@ diag: 66:45 warning "converting the enum constant to a boolean"
//@ diag: 80:42 note "in instantiation of function template specialization 'p<char>' requested here"
//@ diag: 67:38 warning "converting the result of '<<' to a boolean always evaluates to false"
//@ diag: 80:56 note "in instantiation of function template specialization 'q<1>' requested here"
//@ diag: 69:10 warning "converting the result of '<<' to a boolean; did you mean to compare with '0'?"
//@ diag: 80:66 note "in instantiation of function template specialization 'r<int>' requested here"
//@ diag: 69:26 warning "converting the enum constant to a boolean"
//@ diag: 70:15 warning "converting the result of '<<' to a boolean; did you mean to compare with '0'?"
//@ diag: 69:10 warning "converting the result of '<<' to a boolean; did you mean to compare with '0'?"
//@ diag: 81:10 note "in instantiation of function template specialization 'r<char>' requested here"
//@ diag: 69:26 warning "converting the enum constant to a boolean"
//@ diag: 70:15 warning "converting the result of '<<' to a boolean; did you mean to compare with '0'?"
//@ diag: 74:14 warning "converting the result of '<<' to a boolean; did you mean to compare with '0'?"
//@ diag: 81:23 note "in instantiation of function template specialization 's<int>' requested here"
//@ diag: 75:16 warning "converting the result of '<<' to a boolean; did you mean to compare with '0'?"
//@ diag: 77:44 warning "converting the result of '<<' to a boolean; did you mean to compare with '0'?"
//@ diag: 81:36 note "in instantiation of function template specialization 't<1>' requested here"
enum E { A = 2 };
int gv;
template <class T> bool f(T a) {
  if (a << 1) return true;
  return false;
}
template <class T> bool g(int a) {
  if (a << 1) return true;
  return false;
}
template <int N> bool h() { return N << 1; }
template <int N> bool k(int a) { return a ? N : 5; }
template <class T> bool m(T x) { return x && A; }
template <class T> bool p(int x) { return x && A; }
template <int N> bool q() { return 0 << N; }
template <class T> bool r() {
  if (gv << 1) return gv && A;
  bool b = gv << 2;
  return b;
}
template <class T> bool s(int x) {
  bool b = x << 2;
  return b ? x << 3 : false;
}
template <int N> bool t(int x) { return (x << 1) && N; }
bool use() {
  return f(1) || f(2u) || f(3L) || g<int>(1) || g<char>(1) || h<1>() ||
         k<3>(1) || m(1) || p<int>(1) || p<char>(1) || q<1>() || r<int>() ||
         r<char>() || s<int>(1) || t<1>(1);
}
