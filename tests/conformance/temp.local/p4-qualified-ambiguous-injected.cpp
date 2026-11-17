// [temp.local]/4, [class.member.lookup]: a qualified name finding the
// injected-class-names of two specializations through different bases is
// ambiguous unless followed by template arguments; through a dependent
// base `typename D::template B<int>` is looked up at instantiation.
// Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:7 error "member 'Base' found in multiple base classes of different types"
//@ diag: 11:27 note "member type 'Base<void>' found by ambiguous name lookup"
//@ diag: 11:27 note "member type 'Base<void *>' found by ambiguous name lookup"
template <class T> struct Base { };
struct D2: Base<void>, Base<void*> {
  D2::Base* p1;
  D2::Base<double>* p2;
};
