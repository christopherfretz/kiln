// [stmt.switch]/2: a dependent switch condition that is not of integral or
// enumeration type after substitution is diagnosed at instantiation.
//@ expect: compile-fail
//@ diag: 9:3 error "statement requires expression of integer type"
//@ diag: 15 error "duplicate case value"
struct P {};
template <class T>
int f(T t) {
  switch (t) {
  default:
    return 0;
  }
}
template <class T, T A, T B>
int g(T t) { switch (t) { case A: case B: return 1; } return 0; }
int x = f(P{});
int y = g<int, 1, 1>(1);
