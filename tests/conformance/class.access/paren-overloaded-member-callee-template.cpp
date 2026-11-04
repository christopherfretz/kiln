// [class.access]/4, [over.call.func]/3: a parenthesized overloaded member
// name called with dependent arguments, `(k)(T())`, is resolved at
// instantiation with the member function as the access context, so its
// private overloads are accessible (boost::container's
// `(on_capacity_overflow)(throw_on_overflow_t())`).
//@ strict-diags
template <bool B> struct bool_ {};
template <class T, bool Th> class A {
  static int f(bool_<true>) { return 1; }
  static int f(bool_<false>) { return 2; }
  static int k(int) { return 3; }
  static int k(long) { return 4; }
public:
  static int f() { return (f)(bool_<Th>()) + (k)(T()) + ((k))(T()); }
};
int h() { return A<int, true>::f(); }
