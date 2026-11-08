// [temp.inst]/2, [class.mem.general]/8: inside the definition of class
// template `basic`, `basic<long>::jp::string_t` needs `basic<long>`
// complete, which it cannot be while `basic` itself is being defined:
// clang's "implicit instantiation of template 'basic<long>' within its
// own definition" (WP-831, fixed by WP-841). Found with WP-676.
// Verified against clang 19.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:13 error "implicit instantiation of template 'basic<long>' within its own definition"
template <class R> struct ptr { using string_t = R; };
template <class S> struct basic {
  using jp = ptr<S>;
  using t = basic<long>::jp::string_t;
};
basic<int> b;
