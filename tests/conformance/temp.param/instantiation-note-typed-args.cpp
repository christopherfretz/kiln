// [temp.arg.nontype]/1, [temp.inst], [lex.icon]: the "in instantiation
// of function template specialization" note names the specialization
// with its own value arguments typed, as clang's
// `FunctionDecl::getNameForDiagnostic` does (`f<2UL>`, `g<2U>`,
// `h<2LL>`, `sh<(short)3>`, `uc<(unsigned char)'\x03'>`, `C<4>::m<5U>`);
// a class template specialization prints them bare (`S<3>`).
// WP-1844. Verified against clang 22 and clang 19.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 24:52 error "static assertion failed due to requirement '2UL == 0'"
//@ diag: 25:15 note "in instantiation of function template specialization 'f<2UL>' requested here"
//@ diag: 26:47 error "static assertion failed due to requirement '2U == 0'"
//@ diag: 27:15 note "in instantiation of function template specialization 'g<2U>' requested here"
//@ diag: 28:48 error "static assertion failed due to requirement '2LL == 0'"
//@ diag: 29:15 note "in instantiation of function template specialization 'h<2LL>' requested here"
//@ diag: 30:57 error "static assertion failed due to requirement '3ULL == 0'"
//@ diag: 31:6 note "in instantiation of template class 'S<3>' requested here"
//@ diag: 32:45 error "static assertion failed"
//@ diag: 33:15 note "in instantiation of function template specialization 'sh<(short)3>' requested here"
//@ diag: 34:53 error "static assertion failed due to requirement ''\\x03' == 0'"
//@ diag: 35:15 note "in instantiation of function template specialization 'uc<(unsigned char)'\\x03'>' requested here"
//@ diag: 37:49 error "static assertion failed due to requirement '5U == 0'"
//@ diag: 39:21 note "in instantiation of function template specialization 'C<4>::m<5U>' requested here"
template<unsigned long N> void f() { static_assert(N == 0); }
template void f<2>();
template<unsigned N> void g() { static_assert(N == 0); }
template void g<2>();
template<long long N> void h() { static_assert(N == 0); }
template void h<2>();
template<unsigned long long N> struct S { static_assert(N == 0); };
S<3> s;
template<short C> void sh() { static_assert(C == 0); }
template void sh<3>();
template<unsigned char C> void uc() { static_assert(C == 0); }
template void uc<3>();
template<unsigned long N> struct C {
  template<unsigned M> void m() { static_assert(M == 0); }
};
template void C<4>::m<5>();
