// [class.virtual]/2 [dcl.attr.grammar]/5 An override must keep the
// overridden function's calling convention (clang
// `CheckOverridingFunctionAttributes`); clang 22 prints `preserve_most`,
// `preserve_all` and `preserve_none` ahead of a written (attributed) type
// (`'__attribute__((preserve_most)) void ()'`), every other convention
// after it. spec/types.md §5.4, WP-808.
//@ args: --target=x86_64-unknown-linux-gnu
//@ requires: clang>=22
//@ xfail(kiln): WP-808 follow-up, kiln keeps no AttributedType sugar for a written calling convention, so it prints every one after the type (clang 19's form)
//@ expect: compile-fail
//@ strict-diags
//@ diag: 19:21 error "virtual function 'v' has different calling convention attributes ('void ()') than the function it overrides (which has calling convention '__attribute__((preserve_most)) void ()')"
//@ diag: 18:56 note "overridden virtual function is here"
//@ diag: 20:52 error "virtual function 'v' has different calling convention attributes ('__attribute__((preserve_all)) void ()') than the function it overrides (which has calling convention '__attribute__((preserve_most)) void ()')"
//@ diag: 18:56 note "overridden virtual function is here"
//@ diag: 21:46 error "virtual function 'v' has different calling convention attributes ('void () __attribute__((ms_abi))') than the function it overrides (which has calling convention '__attribute__((preserve_most)) void ()')"
//@ diag: 18:56 note "overridden virtual function is here"
struct S { __attribute__((preserve_most)) virtual void v(); };
struct D : S { void v() override; };
struct D2 : S { __attribute__((preserve_all)) void v() override; };
struct D3 : S { __attribute__((ms_abi)) void v() override; };
