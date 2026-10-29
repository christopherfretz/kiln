// [temp.inst]: a trait's completeness check runs when its dependent
// operand is substituted (spec/sema-templates.md §9.1).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 10:30 error "incomplete type 'I' used in type trait expression"
//@ diag: 13:16 note "in instantiation of member function 'E<I>::get' requested here"
//@ diag: 8:8 note "forward declaration of 'I'"
struct I;
template <class T> struct E {
  static bool get() { return __is_empty(T); }
  typedef __remove_cv(T) plain;
};
bool b = E<I>::get();
bool ok = E<int>::get();
E<const I>::plain *p = nullptr;
