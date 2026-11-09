// [class.virtual]/2, [class.dtor]/13: an implicitly declared destructor
// that overrides a virtual destructor with another calling convention is
// diagnosed as a declared one is (clang `CheckOverridingFunctionAttributes`
// from `AddOverriddenMethods`, under "while declaring the implicit
// destructor"), for each derived class: E's base destructor (D's) has the
// default convention. kiln checked only the return type and `final` of
// an implicit overrider. Written after `virtual`, the convention prints
// after the type in clang 19 and 22. spec/sema.md, WP-1103.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 21:8 error "virtual function '~D' has different calling convention attributes ('void ()') than the function it overrides (which has calling convention 'void () __attribute__((preserve_most))')"
//@ diag: 21:8 note "while declaring the implicit destructor for 'D'"
//@ diag: 19:42 note "overridden virtual function is here"
//@ diag: 26:8 error "virtual function '~F' has different calling convention attributes ('void ()') than the function it overrides (which has calling convention 'void () __attribute__((preserve_most)) noexcept')"
//@ diag: 26:8 note "while declaring the implicit destructor for 'F'"
//@ diag: 24:34 note "overridden virtual function is here"

struct B {
  virtual __attribute__((preserve_most)) ~B();
};
struct D : B {};
struct E : D {};
struct C : B {
  __attribute__((preserve_most)) ~C();
};
struct F : C {};
