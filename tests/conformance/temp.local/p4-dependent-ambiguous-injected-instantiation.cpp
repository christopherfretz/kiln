// [temp.local]/4, [temp.res.general]/3: `typename D::B` with `B` found at
// instantiation as the injected-class-names of two specializations in
// different bases is ambiguous.
// Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:16 error "member 'Base' found in multiple base classes of different types"
//@ diag: 16:17 note "in instantiation of template class 'D4<void>' requested here"
//@ diag: 11:27 note "member type 'Base<void>' found by ambiguous name lookup"
//@ diag: 11:27 note "member type 'Base<void *>' found by ambiguous name lookup"
template <class T> struct Base { };
template <class T>
struct D4: Base<T>, Base<T*> {
  typename D4::Base* p1;
};
template struct D4<void>;
