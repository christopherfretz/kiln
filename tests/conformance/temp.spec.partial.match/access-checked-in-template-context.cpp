// [temp.spec.partial.match]/2, [temp.deduct.general]/8: whether a partial
// specialization matches is decided by deduction and substitution, whose
// access checks are done as part of the substitution, not in the context
// that names the specialization: `C::type` is private, so `H<C>` uses the
// primary template even inside a friend of `C`. Found running GCC's C++
// testsuite (WP-1466).
//@ expect: run
template<class, class = void> struct H { static const bool value = false; };
template<class T> struct H<T, typename T::type> { static const bool value = true; };
struct P;
struct C {
private:
  friend struct P;
  typedef void type;
};
struct P {
  static int f() { return H<C>::value ? 1 : 0; }
};
int main() { return P::f(); }
