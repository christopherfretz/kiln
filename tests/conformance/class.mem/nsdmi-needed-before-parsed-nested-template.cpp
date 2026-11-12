// [class.mem.general]/8: the default member initializers of a member class
// template are parsed at the end of the outermost class, so a specialization
// implicitly instantiated by an earlier member declaration has none yet;
// needing one before then (aggregate initialization in a static member
// initializer) is an error.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:35 error "default member initializer for 'v' needed within definition of enclosing class 'Outer' outside of member functions"
//@ diag: 11:35 note "default member initializer declared here"
struct Outer {
  template <class T> struct F { T v = 1; };
  F<int> m;
  static constexpr int k = F<int>{}.v;
};
