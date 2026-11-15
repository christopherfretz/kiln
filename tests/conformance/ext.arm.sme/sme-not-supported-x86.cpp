// [ACLE] Off AArch64 each SME keyword attribute is not supported on this
// target, as clang 19. spec/types.md §5.4, WP-1426.
//@ expect: compile-fail
//@ args: --target=x86_64-unknown-linux-gnu
//@ strict-diags
//@ diag: 15:11 error "'__arm_streaming' is not supported on this target"
//@ diag: 16:1 error "'__arm_locally_streaming' is not supported on this target"
//@ diag: 17:11 error "'__arm_streaming_compatible' is not supported on this target"
//@ diag: 18:1 error "'__arm_new' is not supported on this target"
//@ diag: 19:11 error "'__arm_in' is not supported on this target"
//@ diag: 20:11 error "'__arm_out' is not supported on this target"
//@ diag: 21:11 error "'__arm_inout' is not supported on this target"
//@ diag: 22:11 error "'__arm_preserves' is not supported on this target"
//@ diag: 23:13 error "'__arm_streaming' is not supported on this target"
void s1() __arm_streaming {}
__arm_locally_streaming void s2() {}
void s3() __arm_streaming_compatible {}
__arm_new("za") void s4() {}
void s5() __arm_in("za") {}
void s6() __arm_out("za") {}
void s7() __arm_inout("za") {}
void s8() __arm_preserves("za") {}
void (*p)() __arm_streaming = s1;
