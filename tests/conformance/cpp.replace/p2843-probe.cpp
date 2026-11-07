// P2843R3 ([cpp.replace], [cpp.cond], [cpp.line]; not in cpp.cond/, whose
// kiln_pp-only runner reports no notes): `defined` as a macro name is
// ill-formed; `defined` from macro replacement in #if is ill-formed NDR
// (clang warns); #line 0 and > 2147483647 are conditionally supported.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 11:9 error "'defined' cannot be used as a macro name"
//@ diag: 12:8 warning "undefining builtin macro"
//@ diag: 14:5 warning "macro expansion producing 'defined' has undefined behavior"
//@ diag: 13:11 note "expanded from macro 'D'"
#define defined
#undef __FILE__
#define D defined(X)
#if D
#endif
#line 0
#line 2147483648
