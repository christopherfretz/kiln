// [dcl.attr.deprecated]/4, [temp.inst]/3: a deprecated static data member
// of a class template keeps the attribute in each instantiation (with an
// in-class initializer or not, with an attribute-less out-of-line
// definition), so `C<int>::x` warns, as do its uses inside an instantiated
// initializer; an explicit specialization's own deprecated members and
// enumerators warn too (`numeric_limits<T>::has_denorm`).
//@ expect: compile-ok
//@ strict-diags
//@ diag: 42:17 warning "'x' is deprecated"
//@ diag: 38:5 note "'x' has been explicitly marked deprecated here"
//@ diag: 38:56 warning "'x' is deprecated"
//@ diag: 42:17 note "in instantiation of static data member 'C<int>::x' requested here"
//@ diag: 33:5 note "'x' has been explicitly marked deprecated here"
//@ diag: 43:18 warning "'x' is deprecated"
//@ diag: 38:5 note "'x' has been explicitly marked deprecated here"
//@ diag: 38:56 warning "'x' is deprecated"
//@ diag: 43:18 note "in instantiation of static data member 'C<char>::x' requested here"
//@ diag: 28:5 note "'x' has been explicitly marked deprecated here"
//@ diag: 44:17 warning "'x' is deprecated"
//@ diag: 33:5 note "'x' has been explicitly marked deprecated here"
//@ diag: 45:18 warning "'x' is deprecated"
//@ diag: 28:5 note "'x' has been explicitly marked deprecated here"
//@ diag: 46:18 warning "'z' is deprecated: use y"
//@ diag: 29:5 note "'z' has been explicitly marked deprecated here"
//@ diag: 47:17 warning "'e1' is deprecated"
//@ diag: 34:17 note "'e1' has been explicitly marked deprecated here"
template <class T, bool = true> struct B {
  [[deprecated]] static constexpr const bool x = false;
  [[deprecated("use y")]] static const int z;
};
template <class T, bool b> const int B<T, b>::z = 0;
template <> struct B<int, true> {
  [[deprecated]] static constexpr const bool x = true;
  enum E { e1 [[deprecated]], e2 };
};
template <class T> struct C : B<T> {
  typedef B<T> base;
  [[deprecated]] static constexpr const bool x = base::x;
};
template <class T> constexpr const bool C<T>::x;
void f() {
  (void)C<int>::x;
  (void)C<char>::x;
  (void)B<int>::x;
  (void)B<char>::x;
  (void)B<char>::z;
  (void)B<int>::e1;
  (void)B<int>::e2;
}
