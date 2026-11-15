// [ACLE] The SME keyword attributes as clang 19: where they may appear
// (a type keyword after a parameter clause or a declarator-id, the
// declaration keywords on a function), their states and conflicts, type
// identity (conversions, redeclarations, overriders), `__arm_new` against
// a state of the type, and the ZA / ZT0 state a call needs.
// spec/types.md §5.4, WP-1426.
//@ expect: compile-fail
//@ args: --target=aarch64-unknown-linux-gnu -march=armv9-a+sme2 -ferror-limit=0
//@ strict-diags
//@ diag: 43:1 error "'__arm_streaming' cannot be applied to a declaration"
//@ diag: 44:11 error "'__arm_locally_streaming' cannot be applied to types"
//@ diag: 45:11 error "'__arm_streaming_compatible' and '__arm_streaming' are not compatible"
//@ diag: 45:27 note "conflicting attribute is here"
//@ diag: 49:6 error "function declared 'void ()' was previously declared 'void () __arm_streaming', which has different SME function attributes"
//@ diag: 48:6 note "previous declaration is here"
//@ diag: 51:6 error "function declared 'void () __arm_streaming' was previously declared 'void ()', which has different SME function attributes"
//@ diag: 50:6 note "previous declaration is here"
//@ diag: 52:8 error "cannot initialize a variable of type 'void (*)()' with an lvalue of type 'void () __arm_streaming'"
//@ diag: 53:8 error "cannot initialize a variable of type 'void (*)() __arm_streaming' with an lvalue of type 'void ()'"
//@ diag: 55:21 error "virtual function 'v' has different attributes ('void ()') than the function it overrides (which has 'void () __arm_streaming')"
//@ diag: 54:25 note "overridden virtual function is here"
//@ diag: 55:40 error "virtual function 'w' has different attributes ('void () __arm_streaming') than the function it overrides (which has 'void ()')"
//@ diag: 54:59 note "overridden virtual function is here"
//@ diag: 56:20 error "unknown state 'zb'"
//@ diag: 57:36 error "conflicting attributes for state 'za'"
//@ diag: 58:1 error "'__arm_new(\"za\")' and '__arm_inout(\"za\")' are not compatible"
//@ diag: 60:6 error "function declared 'void () __arm_streaming_compatible' was previously declared 'void () __arm_streaming', which has different SME function attributes"
//@ diag: 59:6 note "previous declaration is here"
//@ diag: 65:12 error "'__arm_streaming' and '__arm_streaming_compatible' are not compatible"
//@ diag: 65:39 note "conflicting attribute is here"
//@ diag: 67:16 error "call to a shared ZT0 function requires the caller to have ZT0 state"
//@ diag: 68:37 error "conflicting attributes for state 'za'"
//@ diag: 69:1 error "'__arm_new(\"za\")' and '__arm_in(\"za\")' are not compatible"
//@ diag: 70:1 error "'__arm_new(\"zt0\")' and '__arm_in(\"zt0\")' are not compatible"
//@ diag: 71:32 error "call to a shared ZT0 function requires the caller to have ZT0 state"
//@ diag: 71:32 error "call to a function that shares state other than 'za' from a function that has live 'za' state requires a spill/fill of ZA, which is not yet implemented"
//@ diag: 71:32 note "add '__arm_preserves(\"za\")' to the callee if it preserves ZA"
//@ diag: 72:12 error "missing state for '__arm_in'"
//@ diag: 73:21 error "expected string literal as argument of '__arm_in' attribute"
//@ diag: 74:11 error "unknown state 'zb'"
//@ diag: 78:7 error "'__arm_streaming' only applies to function types; type here is 'int'"
//@ diag: 79:1 error "'__arm_locally_streaming' only applies to functions"
__arm_streaming void a1();
void a2() __arm_locally_streaming;
void a3() __arm_streaming __arm_streaming_compatible;
__arm_locally_streaming void a4();
__arm_locally_streaming void a4() {}
void a5() __arm_streaming;
void a5() {}
void a6();
void a6() __arm_streaming;
void (*p)() = a5;
void (*q)() __arm_streaming = a6;
struct B { virtual void v() __arm_streaming; virtual void w(); };
struct D : B { void v() override; void w() __arm_streaming override; };
void a7() __arm_in("zb");
void a8() __arm_in("za") __arm_out("za");
__arm_new("za") void a9() __arm_inout("za");
void a10() __arm_streaming;
void a10() __arm_streaming_compatible;
using F = void() __arm_streaming;
F *r = a5;
void c() { r(); }
__arm_locally_streaming void a11() __arm_streaming {}
void a12() __arm_streaming_compatible __arm_streaming;
void z1() __arm_inout("zt0");
void z2() { z1(); }
void a13() __arm_out("za") __arm_in("za", "zt0");
__arm_new("za") void n1() __arm_in("za") {}
__arm_new("zt0") void n2() __arm_in("zt0") {}
__arm_new("za") void n3() { z1(); }
void a14() __arm_in();
void a15() __arm_in(za);
__arm_new("zb") void n4() {}
void a16() __arm_streaming __arm_streaming;
void a17() __arm_in("za") __arm_in("za");
__arm_locally_streaming __arm_locally_streaming void a18() {}
int v __arm_streaming;
__arm_locally_streaming int w;
void (*pp)() __arm_streaming_compatible;
void s1() __arm_streaming_compatible { pp(); }
auto l = []() __arm_streaming {};
void (*pl)() __arm_streaming = l;
