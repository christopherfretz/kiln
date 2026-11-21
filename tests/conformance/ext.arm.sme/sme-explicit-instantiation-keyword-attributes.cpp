// [temp.explicit] [temp.expl.spec] [ACLE] An explicit instantiation or
// specialization of a function template whose type has SME keyword
// attributes (`__arm_streaming`, `__arm_inout("za")`) names them in its own
// type, which is matched against the template's: written, the template
// matches; left out or different, it does not (clang
// `DeduceTemplateArguments` over the function types). Reduced from real
// code: an explicit instantiation of a streaming ZA function template was
// rejected. spec/types.md §5.4 (WP-1792).
//@ expect: compile-fail
//@ args: --target=aarch64-unknown-linux-gnu -march=armv9-a+sme
//@ strict-diags
//@ diag: 25:15 error "explicit instantiation of 's' does not refer to a function template, variable template, member function, member class, or static data member"
//@ diag: 18:23 note "candidate template ignored: could not match 'void () __arm_streaming' against 'void ()'"
//@ diag: 26:18 error "no function template matches function template specialization 'z'"
//@ diag: 22:23 note "candidate template ignored: could not match 'void () __arm_inout("
//@ diag: 27:18 error "no function template matches function template specialization 'z'"
//@ diag: 22:23 note "candidate template ignored: could not match 'void () __arm_inout("
template <int N> void s() __arm_streaming {}
template void s<1>() __arm_streaming;
extern template void s<2>() __arm_streaming;
template <> void s<3>() __arm_streaming {}
template <int N> void z() __arm_inout("za") {}
template void z<1>() __arm_inout("za");
template <> void z<2>() __arm_inout("za") {}
template void s<4>();
template <> void z<3>() {}
template <> void z<4>() __arm_in("za") {}
void (*p)() __arm_streaming = &s<1>;
