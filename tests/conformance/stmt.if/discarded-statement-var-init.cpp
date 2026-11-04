// [stmt.if]/2: clang 20 and earlier give a variable's initializer in a
// discarded statement a potentially-evaluated context of its own, which
// is no longer discarded: what it names is odr-used and instantiated
// ([temp.inst]/8), unlike the rest of the statement (here the expression
// statement's `f(3.0)`). clang 21 and later keep the initializer
// discarded (discarded-statement-var-init-clang22.cpp).
//@ expect: compile-fail
//@ requires: clang<22
//@ strict-diags
//@ diag: 16:17 error "static assertion failed due to requirement 'never<int>': f instantiated"
//@ diag: 21:13 note "in instantiation of function template specialization 'f<int>' requested here"
//@ diag: 16:17 error "static assertion failed due to requirement 'never<long>': f instantiated"
//@ diag: 22:20 note "in instantiation of function template specialization 'f<long>' requested here"
template <class T> inline constexpr bool never = false;
template <class T> int f(T) {
  static_assert(never<T>, "f instantiated");
  return 0;
}
int main() {
  if constexpr (false) {
    int x = f(1);
    static int y = f(2L);
    (void)f(3.0);
    return x + y;
  }
  return 3;
}
