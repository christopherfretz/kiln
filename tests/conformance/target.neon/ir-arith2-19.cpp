// [driver] The rest of the NEON arithmetic builtins the arm_neon.h
// intrinsics reach lower as clang 19's `EmitAArch64BuiltinExpr`, for every
// type the type code allows: `vshl_n` a `shl`, `vshrn_n` a shift of the
// wide vector then `trunc`, `vsra_n` an `add` of the `vshr_n` shift (zero
// or width - 1 at full width), `vsri_n` a `vsri` call, `vmovl`
// `sext`/`zext`, `vqmovn` `{s,u}qxtn`, `vfma_lane[q]` an `fma` of a lane
// splat (`vfma_laneq_f64` the scalar `fma.f64`), the `vcvt` float width
// conversions, and one call each for `vpmin`, `vpmax`, `vabd`, `vqadd`,
// `vminnm`, `vmaxnm`, `vrecpe`, `vrecps`, `vrsqrte`, `vrsqrts`, `vrnd*`,
// `vcnt`, `vclz`, `vcmla*`; the `_f16` names are their `_v` twins
// (spec/landed/codegen/WP-628.md "NEON arithmetic builtins"). WP-628.
//@ requires: clang<22
//@ requires: target-aarch64
//@ expect: ir
//@ args: -march=armv8.3-a+fp16 -ftrivial-auto-var-init=uninitialized
#include <arm_neon.h>
auto f_vshlq_n_u8(uint8x16_t a0) { return vshlq_n_u8(a0, 3); }
auto f_vshlq_n_u32(uint32x4_t a0) { return vshlq_n_u32(a0, 3); }
auto f_vshlq_n_u64(uint64x2_t a0) { return vshlq_n_u64(a0, 3); }
auto f_vshlq_n_u16(uint16x8_t a0) { return vshlq_n_u16(a0, 3); }
auto f_vshlq_n_s8(int8x16_t a0) { return vshlq_n_s8(a0, 3); }
auto f_vshlq_n_s32(int32x4_t a0) { return vshlq_n_s32(a0, 3); }
auto f_vshlq_n_s64(int64x2_t a0) { return vshlq_n_s64(a0, 3); }
auto f_vshlq_n_s16(int16x8_t a0) { return vshlq_n_s16(a0, 3); }
auto f_vshl_n_u8(uint8x8_t a0) { return vshl_n_u8(a0, 3); }
auto f_vshl_n_u32(uint32x2_t a0) { return vshl_n_u32(a0, 3); }
auto f_vshl_n_u64(uint64x1_t a0) { return vshl_n_u64(a0, 3); }
auto f_vshl_n_u16(uint16x4_t a0) { return vshl_n_u16(a0, 3); }
auto f_vshl_n_s8(int8x8_t a0) { return vshl_n_s8(a0, 3); }
auto f_vshl_n_s32(int32x2_t a0) { return vshl_n_s32(a0, 3); }
auto f_vshl_n_s64(int64x1_t a0) { return vshl_n_s64(a0, 3); }
auto f_vshl_n_s16(int16x4_t a0) { return vshl_n_s16(a0, 3); }
auto f_vshrn_n_u32(uint32x4_t a0) { return vshrn_n_u32(a0, 3); }
auto f_vshrn_n_u64(uint64x2_t a0) { return vshrn_n_u64(a0, 3); }
auto f_vshrn_n_u16(uint16x8_t a0) { return vshrn_n_u16(a0, 3); }
auto f_vshrn_n_s32(int32x4_t a0) { return vshrn_n_s32(a0, 3); }
auto f_vshrn_n_s64(int64x2_t a0) { return vshrn_n_s64(a0, 3); }
auto f_vshrn_n_s16(int16x8_t a0) { return vshrn_n_s16(a0, 3); }
auto f_vsraq_n_u8(uint8x16_t a0, uint8x16_t a1) { return vsraq_n_u8(a0, a1, 3); }
auto f_vsraq_n_u32(uint32x4_t a0, uint32x4_t a1) { return vsraq_n_u32(a0, a1, 3); }
auto f_vsraq_n_u64(uint64x2_t a0, uint64x2_t a1) { return vsraq_n_u64(a0, a1, 3); }
auto f_vsraq_n_u16(uint16x8_t a0, uint16x8_t a1) { return vsraq_n_u16(a0, a1, 3); }
auto f_vsraq_n_s8(int8x16_t a0, int8x16_t a1) { return vsraq_n_s8(a0, a1, 3); }
auto f_vsraq_n_s32(int32x4_t a0, int32x4_t a1) { return vsraq_n_s32(a0, a1, 3); }
auto f_vsraq_n_s64(int64x2_t a0, int64x2_t a1) { return vsraq_n_s64(a0, a1, 3); }
auto f_vsraq_n_s16(int16x8_t a0, int16x8_t a1) { return vsraq_n_s16(a0, a1, 3); }
auto f_vsra_n_u8(uint8x8_t a0, uint8x8_t a1) { return vsra_n_u8(a0, a1, 3); }
auto f_vsra_n_u32(uint32x2_t a0, uint32x2_t a1) { return vsra_n_u32(a0, a1, 3); }
auto f_vsra_n_u64(uint64x1_t a0, uint64x1_t a1) { return vsra_n_u64(a0, a1, 3); }
auto f_vsra_n_u16(uint16x4_t a0, uint16x4_t a1) { return vsra_n_u16(a0, a1, 3); }
auto f_vsra_n_s8(int8x8_t a0, int8x8_t a1) { return vsra_n_s8(a0, a1, 3); }
auto f_vsra_n_s32(int32x2_t a0, int32x2_t a1) { return vsra_n_s32(a0, a1, 3); }
auto f_vsra_n_s64(int64x1_t a0, int64x1_t a1) { return vsra_n_s64(a0, a1, 3); }
auto f_vsra_n_s16(int16x4_t a0, int16x4_t a1) { return vsra_n_s16(a0, a1, 3); }
auto f_vsri_n_p8(poly8x8_t a0, poly8x8_t a1) { return vsri_n_p8(a0, a1, 3); }
auto f_vsri_n_p16(poly16x4_t a0, poly16x4_t a1) { return vsri_n_p16(a0, a1, 3); }
auto f_vsriq_n_p8(poly8x16_t a0, poly8x16_t a1) { return vsriq_n_p8(a0, a1, 3); }
auto f_vsriq_n_p16(poly16x8_t a0, poly16x8_t a1) { return vsriq_n_p16(a0, a1, 3); }
auto f_vsriq_n_u8(uint8x16_t a0, uint8x16_t a1) { return vsriq_n_u8(a0, a1, 3); }
auto f_vsriq_n_u32(uint32x4_t a0, uint32x4_t a1) { return vsriq_n_u32(a0, a1, 3); }
auto f_vsriq_n_u64(uint64x2_t a0, uint64x2_t a1) { return vsriq_n_u64(a0, a1, 3); }
auto f_vsriq_n_u16(uint16x8_t a0, uint16x8_t a1) { return vsriq_n_u16(a0, a1, 3); }
auto f_vsriq_n_s8(int8x16_t a0, int8x16_t a1) { return vsriq_n_s8(a0, a1, 3); }
auto f_vsriq_n_s32(int32x4_t a0, int32x4_t a1) { return vsriq_n_s32(a0, a1, 3); }
auto f_vsriq_n_s64(int64x2_t a0, int64x2_t a1) { return vsriq_n_s64(a0, a1, 3); }
auto f_vsriq_n_s16(int16x8_t a0, int16x8_t a1) { return vsriq_n_s16(a0, a1, 3); }
auto f_vsri_n_u8(uint8x8_t a0, uint8x8_t a1) { return vsri_n_u8(a0, a1, 3); }
auto f_vsri_n_u32(uint32x2_t a0, uint32x2_t a1) { return vsri_n_u32(a0, a1, 3); }
auto f_vsri_n_u64(uint64x1_t a0, uint64x1_t a1) { return vsri_n_u64(a0, a1, 3); }
auto f_vsri_n_u16(uint16x4_t a0, uint16x4_t a1) { return vsri_n_u16(a0, a1, 3); }
auto f_vsri_n_s8(int8x8_t a0, int8x8_t a1) { return vsri_n_s8(a0, a1, 3); }
auto f_vsri_n_s32(int32x2_t a0, int32x2_t a1) { return vsri_n_s32(a0, a1, 3); }
auto f_vsri_n_s64(int64x1_t a0, int64x1_t a1) { return vsri_n_s64(a0, a1, 3); }
auto f_vsri_n_s16(int16x4_t a0, int16x4_t a1) { return vsri_n_s16(a0, a1, 3); }
auto f_vsri_n_p64(poly64x1_t a0, poly64x1_t a1) { return vsri_n_p64(a0, a1, 3); }
auto f_vsriq_n_p64(poly64x2_t a0, poly64x2_t a1) { return vsriq_n_p64(a0, a1, 3); }
auto f_vfmaq_lane_f16(float16x8_t a0, float16x8_t a1, float16x4_t a2) { return vfmaq_lane_f16(a0, a1, a2, 1); }
auto f_vfma_lane_f16(float16x4_t a0, float16x4_t a1, float16x4_t a2) { return vfma_lane_f16(a0, a1, a2, 1); }
auto f_vfmaq_laneq_f16(float16x8_t a0, float16x8_t a1, float16x8_t a2) { return vfmaq_laneq_f16(a0, a1, a2, 1); }
auto f_vfma_laneq_f16(float16x4_t a0, float16x4_t a1, float16x8_t a2) { return vfma_laneq_f16(a0, a1, a2, 1); }
auto f_vfmaq_lane_f64(float64x2_t a0, float64x2_t a1, float64x1_t a2) { return vfmaq_lane_f64(a0, a1, a2, 1); }
auto f_vfmaq_lane_f32(float32x4_t a0, float32x4_t a1, float32x2_t a2) { return vfmaq_lane_f32(a0, a1, a2, 1); }
auto f_vfma_lane_f64(float64x1_t a0, float64x1_t a1, float64x1_t a2) { return vfma_lane_f64(a0, a1, a2, 0); }
auto f_vfma_lane_f32(float32x2_t a0, float32x2_t a1, float32x2_t a2) { return vfma_lane_f32(a0, a1, a2, 1); }
auto f_vfmaq_laneq_f64(float64x2_t a0, float64x2_t a1, float64x2_t a2) { return vfmaq_laneq_f64(a0, a1, a2, 1); }
auto f_vfmaq_laneq_f32(float32x4_t a0, float32x4_t a1, float32x4_t a2) { return vfmaq_laneq_f32(a0, a1, a2, 1); }
auto f_vfma_laneq_f64(float64x1_t a0, float64x1_t a1, float64x2_t a2) { return vfma_laneq_f64(a0, a1, a2, 1); }
auto f_vfma_laneq_f32(float32x2_t a0, float32x2_t a1, float32x4_t a2) { return vfma_laneq_f32(a0, a1, a2, 1); }
auto p0 = &vpmax_f16;
auto p1 = &vpmin_f16;
auto p2 = &vpmax_u8;
auto p3 = &vpmax_u32;
auto p4 = &vpmax_u16;
auto p5 = &vpmax_s8;
auto p6 = &vpmax_f32;
auto p7 = &vpmax_s32;
auto p8 = &vpmax_s16;
auto p9 = &vpmin_u8;
auto p10 = &vpmin_u32;
auto p11 = &vpmin_u16;
auto p12 = &vpmin_s8;
auto p13 = &vpmin_f32;
auto p14 = &vpmin_s32;
auto p15 = &vpmin_s16;
auto p16 = &vpmaxq_f16;
auto p17 = &vpminq_f16;
auto p18 = &vpmaxq_u8;
auto p19 = &vpmaxq_u32;
auto p20 = &vpmaxq_u16;
auto p21 = &vpmaxq_s8;
auto p22 = &vpmaxq_f64;
auto p23 = &vpmaxq_f32;
auto p24 = &vpmaxq_s32;
auto p25 = &vpmaxq_s16;
auto p26 = &vpminq_u8;
auto p27 = &vpminq_u32;
auto p28 = &vpminq_u16;
auto p29 = &vpminq_s8;
auto p30 = &vpminq_f64;
auto p31 = &vpminq_f32;
auto p32 = &vpminq_s32;
auto p33 = &vpminq_s16;
auto p34 = &vabdq_f16;
auto p35 = &vabd_f16;
auto p36 = &vabdq_u8;
auto p37 = &vabdq_u32;
auto p38 = &vabdq_u16;
auto p39 = &vabdq_s8;
auto p40 = &vabdq_f32;
auto p41 = &vabdq_s32;
auto p42 = &vabdq_s16;
auto p43 = &vabd_u8;
auto p44 = &vabd_u32;
auto p45 = &vabd_u16;
auto p46 = &vabd_s8;
auto p47 = &vabd_f32;
auto p48 = &vabd_s32;
auto p49 = &vabd_s16;
auto p50 = &vabdq_f64;
auto p51 = &vabd_f64;
auto p52 = &vmovl_u8;
auto p53 = &vmovl_u32;
auto p54 = &vmovl_u16;
auto p55 = &vmovl_s8;
auto p56 = &vmovl_s32;
auto p57 = &vmovl_s16;
auto p58 = &vrecpeq_f16;
auto p59 = &vrecpe_f16;
auto p60 = &vrecpsq_f16;
auto p61 = &vrecps_f16;
auto p62 = &vrsqrteq_f16;
auto p63 = &vrsqrte_f16;
auto p64 = &vrsqrtsq_f16;
auto p65 = &vrsqrts_f16;
auto p66 = &vrecpeq_u32;
auto p67 = &vrecpeq_f32;
auto p68 = &vrecpe_u32;
auto p69 = &vrecpe_f32;
auto p70 = &vrecpsq_f32;
auto p71 = &vrecps_f32;
auto p72 = &vrsqrteq_u32;
auto p73 = &vrsqrteq_f32;
auto p74 = &vrsqrte_u32;
auto p75 = &vrsqrte_f32;
auto p76 = &vrsqrtsq_f32;
auto p77 = &vrsqrts_f32;
auto p78 = &vrecpeq_f64;
auto p79 = &vrecpe_f64;
auto p80 = &vrecpsq_f64;
auto p81 = &vrecps_f64;
auto p82 = &vrsqrteq_f64;
auto p83 = &vrsqrte_f64;
auto p84 = &vrsqrtsq_f64;
auto p85 = &vrsqrts_f64;
auto p86 = &vrndq_f64;
auto p87 = &vrnd_f64;
auto p88 = &vrndaq_f64;
auto p89 = &vrnda_f64;
auto p90 = &vrndmq_f64;
auto p91 = &vrndm_f64;
auto p92 = &vrndnq_f64;
auto p93 = &vrndn_f64;
auto p94 = &vrndpq_f64;
auto p95 = &vrndp_f64;
auto p96 = &vrndq_f16;
auto p97 = &vrnd_f16;
auto p98 = &vrndaq_f16;
auto p99 = &vrnda_f16;
auto p100 = &vrndmq_f16;
auto p101 = &vrndm_f16;
auto p102 = &vrndnq_f16;
auto p103 = &vrndn_f16;
auto p104 = &vrndpq_f16;
auto p105 = &vrndp_f16;
auto p106 = &vrndq_f32;
auto p107 = &vrnd_f32;
auto p108 = &vrndaq_f32;
auto p109 = &vrnda_f32;
auto p110 = &vrndmq_f32;
auto p111 = &vrndm_f32;
auto p112 = &vrndnq_f32;
auto p113 = &vrndn_f32;
auto p114 = &vrndpq_f32;
auto p115 = &vrndp_f32;
auto p116 = &vmaxnmq_f64;
auto p117 = &vmaxnm_f64;
auto p118 = &vminnmq_f64;
auto p119 = &vminnm_f64;
auto p120 = &vmaxnmq_f16;
auto p121 = &vmaxnm_f16;
auto p122 = &vminnmq_f16;
auto p123 = &vminnm_f16;
auto p124 = &vmaxnmq_f32;
auto p125 = &vmaxnm_f32;
auto p126 = &vminnmq_f32;
auto p127 = &vminnm_f32;
auto p128 = &vcvt_f16_f32;
auto p129 = &vcvt_f32_f16;
auto p130 = &vcvt_f32_f64;
auto p131 = &vcvt_f64_f32;
auto p132 = &vclzq_u8;
auto p133 = &vclzq_u32;
auto p134 = &vclzq_u16;
auto p135 = &vclzq_s8;
auto p136 = &vclzq_s32;
auto p137 = &vclzq_s16;
auto p138 = &vclz_u8;
auto p139 = &vclz_u32;
auto p140 = &vclz_u16;
auto p141 = &vclz_s8;
auto p142 = &vclz_s32;
auto p143 = &vclz_s16;
auto p144 = &vcnt_p8;
auto p145 = &vcntq_p8;
auto p146 = &vcntq_u8;
auto p147 = &vcntq_s8;
auto p148 = &vcnt_u8;
auto p149 = &vcnt_s8;
auto p150 = &vqaddq_u8;
auto p151 = &vqaddq_u32;
auto p152 = &vqaddq_u64;
auto p153 = &vqaddq_u16;
auto p154 = &vqaddq_s8;
auto p155 = &vqaddq_s32;
auto p156 = &vqaddq_s64;
auto p157 = &vqaddq_s16;
auto p158 = &vqadd_u8;
auto p159 = &vqadd_u32;
auto p160 = &vqadd_u64;
auto p161 = &vqadd_u16;
auto p162 = &vqadd_s8;
auto p163 = &vqadd_s32;
auto p164 = &vqadd_s64;
auto p165 = &vqadd_s16;
auto p166 = &vqmovn_u32;
auto p167 = &vqmovn_u64;
auto p168 = &vqmovn_u16;
auto p169 = &vqmovn_s32;
auto p170 = &vqmovn_s64;
auto p171 = &vqmovn_s16;
auto p172 = &vcmlaq_f16;
auto p173 = &vcmla_f16;
auto p174 = &vcmlaq_rot180_f16;
auto p175 = &vcmla_rot180_f16;
auto p176 = &vcmlaq_rot270_f16;
auto p177 = &vcmla_rot270_f16;
auto p178 = &vcmlaq_rot90_f16;
auto p179 = &vcmla_rot90_f16;
auto p180 = &vcmlaq_f32;
auto p181 = &vcmla_f32;
auto p182 = &vcmlaq_rot180_f32;
auto p183 = &vcmla_rot180_f32;
auto p184 = &vcmlaq_rot270_f32;
auto p185 = &vcmla_rot270_f32;
auto p186 = &vcmlaq_rot90_f32;
auto p187 = &vcmla_rot90_f32;
auto p188 = &vcmlaq_f64;
auto p189 = &vcmlaq_rot180_f64;
auto p190 = &vcmlaq_rot270_f64;
auto p191 = &vcmlaq_rot90_f64;
auto g_vsra_n_u8(uint8x8_t a, uint8x8_t b) { return vsra_n_u8(a, b, 8); }
auto g_vsra_n_s8(int8x8_t a, int8x8_t b) { return vsra_n_s8(a, b, 8); }
auto g_vsraq_n_u64(uint64x2_t a, uint64x2_t b) { return vsraq_n_u64(a, b, 64); }
auto g_vshl_n_u8(uint8x8_t a) { return vshl_n_u8(a, 0); }
auto g_vshlq_n_s64(int64x2_t a) { return vshlq_n_s64(a, 63); }
auto g_vshrn_n_u16(uint16x8_t a) { return vshrn_n_u16(a, 8); }
auto g_vsri_n_u8(uint8x8_t a, uint8x8_t b) { return vsri_n_u8(a, b, 8); }
auto g_vsriq_n_p64(poly64x2_t a, poly64x2_t b) { return vsriq_n_p64(a, b, 64); }
auto g_vfma_lane_f32(float32x2_t a, float32x2_t b, float32x2_t c) { return vfma_lane_f32(a, b, c, 0); }
auto g_vfmaq_laneq_f32(float32x4_t a, float32x4_t b, float32x4_t c) { return vfmaq_laneq_f32(a, b, c, 3); }
auto g_vfma_laneq_f64(float64x1_t a, float64x1_t b, float64x2_t c) { return vfma_laneq_f64(a, b, c, 0); }
