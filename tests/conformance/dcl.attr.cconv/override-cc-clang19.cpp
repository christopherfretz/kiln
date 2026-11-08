// [class.virtual]/2 [dcl.attr.grammar]/5 An override must keep the
// overridden function's calling convention (clang
// `CheckOverridingFunctionAttributes`); clang 19 prints the convention
// after the type (`'void () __attribute__((preserve_most))'`).
// spec/types.md §5.4, WP-808.
//@ args: --target=x86_64-unknown-linux-gnu
//@ requires: clang<22
//@ expect: compile-fail
//@ strict-diags
//@ diag: 17:21 error "virtual function 'v' has different calling convention attributes ('void ()') than the function it overrides (which has calling convention 'void () __attribute__((preserve_most))')"
//@ diag: 16:56 note "overridden virtual function is here"
//@ diag: 18:52 error "virtual function 'v' has different calling convention attributes ('void () __attribute__((preserve_all))') than the function it overrides (which has calling convention 'void () __attribute__((preserve_most))')"
//@ diag: 16:56 note "overridden virtual function is here"
//@ diag: 19:46 error "virtual function 'v' has different calling convention attributes ('void () __attribute__((ms_abi))') than the function it overrides (which has calling convention 'void () __attribute__((preserve_most))')"
//@ diag: 16:56 note "overridden virtual function is here"
struct S { __attribute__((preserve_most)) virtual void v(); };
struct D : S { void v() override; };
struct D2 : S { __attribute__((preserve_all)) void v() override; };
struct D3 : S { __attribute__((ms_abi)) void v() override; };
