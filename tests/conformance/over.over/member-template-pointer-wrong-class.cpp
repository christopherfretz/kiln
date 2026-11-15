// [over.over]/1, [temp.arg.explicit]: in a variable template, `&C::m<u>`
// with a dependent explicit argument initializing a pointer to member of
// another class is ill-formed. kiln retargeted the overload set to the
// required type again and again and overflowed the stack; it now
// retargets once. Found running GCC's C++ testsuite (WP-1456). clang
// says "cannot initialize a variable of type ... different classes" at
// the `&`; kiln says the address of the overloaded function does not
// match the required type, at `m` (wording not pinned).
//@ expect: compile-fail
//@ diag: 15 error ""
template<class> struct H;
struct C { template<int> void m(); };
template<> struct H<C> {
  using T = void (H::*)();
  template<int u> static constexpr T h = &C::m<u>;
};
void use() { H<C>::h<0>; }
