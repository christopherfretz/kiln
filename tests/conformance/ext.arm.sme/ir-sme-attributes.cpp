// [ACLE] The SME keyword attributes in IR as clang 19 and 22: the
// streaming mode and the ZA / ZT0 state of a function type are string
// attributes of its definitions, declarations and calls (direct, through
// a pointer, virtual, and a lambda's call operator);
// `__arm_locally_streaming` and `__arm_new` those of the function
// declaration. spec/types.md §5.4, WP-1426.
// `-mno-outline-atomics`: the features must not depend on the host's
// sysroot; `-ftrivial-auto-var-init=uninitialized`: clang 22 has no C++26
// erroneous values. No header is included under the pinned --target.
//@ expect: ir
//@ args: --target=aarch64-unknown-linux-gnu -march=armv9-a+sme2 -mno-outline-atomics -ftrivial-auto-var-init=uninitialized
void s1() __arm_streaming {}
__arm_locally_streaming void s2() {}
void s3() __arm_streaming_compatible {}
__arm_new("za") void s4() {}
void s5() __arm_in("za") {}
void s6() __arm_out("za") {}
void s7() __arm_inout("za") {}
void s8() __arm_preserves("za") {}
void s10() __arm_inout("za", "zt0") {}
__arm_new("zt0") void s11() {}
void s12() __arm_streaming __arm_inout("za") {}
void c1() __arm_inout("za") { s1(); s3(); s5(); s12(); }
void (*p1)() __arm_streaming = s1;
void d1() __arm_streaming;
void d2() __arm_inout("za");
__arm_locally_streaming void d3();
void c2() __arm_streaming_compatible __arm_inout("za") { d1(); d2(); d3(); p1(); }
struct B { virtual void v() __arm_streaming; virtual void w(); };
void B::v() __arm_streaming {}
void c3(B &b) __arm_inout("za") { b.v(); auto l = []() __arm_streaming {}; l(); }
