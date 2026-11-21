// [temp.expl.spec] [clang ext] Of the function type attributes, only the
// calling convention (and GNU `noreturn`) of a function template is
// adopted when an explicit specialization is matched (clang
// `adjustCCAndNoReturn`): one that adds or drops
// `no_caller_saved_registers` or `regparm` matches no template, and one
// naming another convention is a redeclaration's mismatch. Reduced from
// real code. WP-1843.
//@ expect: compile-fail
//@ args: --target=x86_64-unknown-linux-gnu
//@ strict-diags
//@ diag: 23:18 error "no function template matches function template specialization 'h'"
//@ diag: 22:26 note "candidate template ignored: could not match 'void () __attribute__((no_caller_saved_registers))' against 'void ()'"
//@ diag: 26:21 error "no function template matches function template specialization 'k'"
//@ diag: 25:23 note "candidate template ignored: could not match 'void ()' against 'void () __attribute__((no_caller_saved_registers))'"
//@ diag: 28:18 error "no function template matches function template specialization 'r'"
//@ diag: 27:51 note "candidate template ignored: could not match 'void () __attribute__((regparm (2)))' against 'void ()'"
//@ diag: 30:46 error "no function template matches function template specialization 'r2'"
//@ diag: 29:23 note "candidate template ignored: could not match 'void ()' against 'void () __attribute__((regparm (2)))'"
//@ diag: 35:42 error "function declared 'ms_abi' here was previously declared without calling convention"
//@ diag: 35:42 note "previous declaration is here"
#define NC __attribute__((no_caller_saved_registers))
template <int N> NC void h() {}
template <> void h<1>() {}
template <> NC void h<2>() {}
template <int N> void k() {}
template <> NC void k<1>() {}
template <int N> __attribute__((regparm(2))) void r() {}
template <> void r<1>() {}
template <int N> void r2() {}
template <> __attribute__((regparm(2))) void r2<1>() {}
template <int N> __attribute__((ms_abi)) void ms() {}
template <> void ms<2>() {}
template <> __attribute__((ms_abi)) void ms<3>() {}
template <int N> void sv() {}
template <> __attribute__((ms_abi)) void sv<1>() {}
