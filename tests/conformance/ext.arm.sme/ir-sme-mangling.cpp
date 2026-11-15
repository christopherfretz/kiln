// [ACLE] [AAPCS64] The SME attributes of a function type are mangled as
// clang 20 and later do: `11__SME_ATTRSI <function-type> Lj<bits>E E`,
// with the AAPCS64 bitmask (streaming 1, streaming-compatible 2, the ZA
// state at bit 3 and the ZT0 state at bit 6: in 1, out 2, inout 3,
// preserves 4). clang 19 mangles none of them, so the specializations
// below would collide there. spec/types.md §5.4, WP-1426.
//@ expect: ir
//@ min-oracle: 22
//@ args: --target=aarch64-unknown-linux-gnu -march=armv9-a+sme2 -mno-outline-atomics -ftrivial-auto-var-init=uninitialized
void s1() __arm_streaming {}
void s3() __arm_streaming_compatible {}
void s5() __arm_in("za") {}
void s6() __arm_out("za") {}
void s7() __arm_inout("za") {}
void s8() __arm_preserves("za") {}
void s10() __arm_inout("za", "zt0") {}
void s12() __arm_streaming __arm_inout("za") {}
template<class T> void tt(T) {}
void m() __arm_inout("za", "zt0") {
  tt(s1); tt(s3); tt(s5); tt(s6); tt(s7); tt(s8); tt(s10); tt(s12);
}
void g(void (*)() __arm_streaming, void (*)() __arm_streaming) {}
void h(void (&)(int) __arm_streaming_compatible) {}
