// [stmt.if]/2: outside a template a discarded statement is not
// instantiated away, only not odr-used; it is still fully checked, at any
// nesting depth, so non-dependent ill-formed code in it is diagnosed as
// anywhere else: an undeclared name, a bad conversion, a call no
// function template matches, a deleted function, an inaccessible member
// and a failing static_assert. (The function templates named there are
// still not instantiated: `g`'s body would fail its own static_assert.)
//@ expect: compile-fail
//@ strict-diags
//@ diag: 32:11 error "use of undeclared identifier 'undeclared'"
//@ diag: 33:10 error "cannot initialize a variable of type 'int *' with an rvalue of type 'double'"
//@ diag: 34:11 error "no matching function for call to 'g'"
//@ diag: 22:24 note "candidate template ignored: deduced conflicting types for parameter 'T' ('int' vs. 'long')"
//@ diag: 35:5 error "call to deleted function 'del'"
//@ diag: 26:6 note "candidate function has been explicitly deleted"
//@ diag: 36:13 error "'priv' is a private member of 'P'"
//@ diag: 28:7 note "implicitly declared private here"
//@ diag: 37:19 error "static assertion failed due to requirement 'sizeof(int) == 0': still evaluated"
//@ diag: 37:31 note "expression evaluates to '4 == 0'"
//@ diag: 40:14 error "use of undeclared identifier 'also_undeclared'"
template <class T> inline constexpr bool never = false;
template <class T> int g(T, T) {
  static_assert(never<T>, "g instantiated");
  return 0;
}
void del() = delete;
class P {
  int priv = 0;
};
int use(P &p) {
  if constexpr (false) {
    (void)undeclared;
    int *q = 1.5;
    (void)g(1, 2L);
    del();
    (void)p.priv;
    static_assert(sizeof(int) == 0, "still evaluated");
    if constexpr (false) {
      (void)g(1, 2);
      return also_undeclared;
    }
  }
  return 0;
}
