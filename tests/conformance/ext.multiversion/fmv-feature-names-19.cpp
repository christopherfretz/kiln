// [clang ext] The AArch64 function multiversioning feature names that
// `target_version` accepts (clang 19 `AArch64TargetInfo::validateCpuSupports`
// list, ACLE "Function Multi Versioning"); every other name is warned
// about and the attribute ignored. spec/landed/codegen/WP-810.md (WP-810).
//@ expect: compile-ok
//@ requires: clang<22
//@ args: --target=aarch64-unknown-linux-gnu
//@ strict-diags
//@ diag: 88:31 warning "unsupported 'cssc' in the 'target_version' attribute string; 'target_version' attribute ignored"
//@ diag: 89:31 warning "unsupported 'neon' in the 'target_version' attribute string; 'target_version' attribute ignored"
//@ diag: 90:31 warning "unsupported 'crypto' in the 'target_version' attribute string; 'target_version' attribute ignored"
//@ diag: 91:31 warning "unsupported 'lse128' in the 'target_version' attribute string; 'target_version' attribute ignored"
//@ diag: 92:31 warning "unsupported 'sve2p1' in the 'target_version' attribute string; 'target_version' attribute ignored"
//@ diag: 93:31 warning "unsupported 'sme2p1' in the 'target_version' attribute string; 'target_version' attribute ignored"
//@ diag: 94:31 warning "unsupported 'fp8' in the 'target_version' attribute string; 'target_version' attribute ignored"
//@ diag: 95:31 warning "unsupported 'faminmax' in the 'target_version' attribute string; 'target_version' attribute ignored"
//@ diag: 96:31 warning "unsupported 'lut' in the 'target_version' attribute string; 'target_version' attribute ignored"
//@ diag: 97:31 warning "unsupported 'gcs' in the 'target_version' attribute string; 'target_version' attribute ignored"
//@ diag: 98:31 warning "unsupported 'mte' in the 'target_version' attribute string; 'target_version' attribute ignored"
//@ diag: 99:31 warning "unsupported 'rcpc-immo' in the 'target_version' attribute string; 'target_version' attribute ignored"
//@ diag: 100:31 warning "unsupported 'the' in the 'target_version' attribute string; 'target_version' attribute ignored"
//@ diag: 101:31 warning "unsupported 'd128' in the 'target_version' attribute string; 'target_version' attribute ignored"
//@ diag: 102:31 warning "unsupported 'v8.1a' in the 'target_version' attribute string; 'target_version' attribute ignored"
//@ diag: 103:31 warning "unsupported 'sha512' in the 'target_version' attribute string; 'target_version' attribute ignored"
//@ diag: 104:31 warning "unsupported 'sm3' in the 'target_version' attribute string; 'target_version' attribute ignored"
//@ diag: 105:31 warning "unsupported 'sme-f16f16' in the 'target_version' attribute string; 'target_version' attribute ignored"
//@ diag: 107:31 warning "unsupported 'cpa' in the 'target_version' attribute string; 'target_version' attribute ignored"
__attribute__((target_version("rng"))) int f1();
__attribute__((target_version("flagm"))) int f2();
__attribute__((target_version("flagm2"))) int f3();
__attribute__((target_version("fp16fml"))) int f4();
__attribute__((target_version("dotprod"))) int f5();
__attribute__((target_version("sm4"))) int f6();
__attribute__((target_version("rdm"))) int f7();
__attribute__((target_version("lse"))) int f8();
__attribute__((target_version("fp"))) int f9();
__attribute__((target_version("simd"))) int f10();
__attribute__((target_version("crc"))) int f11();
__attribute__((target_version("sha1"))) int f12();
__attribute__((target_version("sha2"))) int f13();
__attribute__((target_version("sha3"))) int f14();
__attribute__((target_version("aes"))) int f15();
__attribute__((target_version("pmull"))) int f16();
__attribute__((target_version("fp16"))) int f17();
__attribute__((target_version("dit"))) int f18();
__attribute__((target_version("dpb"))) int f19();
__attribute__((target_version("dpb2"))) int f20();
__attribute__((target_version("jscvt"))) int f21();
__attribute__((target_version("fcma"))) int f22();
__attribute__((target_version("rcpc"))) int f23();
__attribute__((target_version("rcpc2"))) int f24();
__attribute__((target_version("rcpc3"))) int f25();
__attribute__((target_version("frintts"))) int f26();
__attribute__((target_version("dgh"))) int f27();
__attribute__((target_version("i8mm"))) int f28();
__attribute__((target_version("bf16"))) int f29();
__attribute__((target_version("ebf16"))) int f30();
__attribute__((target_version("rpres"))) int f31();
__attribute__((target_version("sve"))) int f32();
__attribute__((target_version("sve-bf16"))) int f33();
__attribute__((target_version("sve-ebf16"))) int f34();
__attribute__((target_version("sve-i8mm"))) int f35();
__attribute__((target_version("f32mm"))) int f36();
__attribute__((target_version("f64mm"))) int f37();
__attribute__((target_version("sve2"))) int f38();
__attribute__((target_version("sve2-aes"))) int f39();
__attribute__((target_version("sve2-pmull128"))) int f40();
__attribute__((target_version("sve2-bitperm"))) int f41();
__attribute__((target_version("sve2-sha3"))) int f42();
__attribute__((target_version("sve2-sm4"))) int f43();
__attribute__((target_version("sme"))) int f44();
__attribute__((target_version("memtag"))) int f45();
__attribute__((target_version("memtag2"))) int f46();
__attribute__((target_version("memtag3"))) int f47();
__attribute__((target_version("sb"))) int f48();
__attribute__((target_version("predres"))) int f49();
__attribute__((target_version("ssbs"))) int f50();
__attribute__((target_version("ssbs2"))) int f51();
__attribute__((target_version("bti"))) int f52();
__attribute__((target_version("ls64"))) int f53();
__attribute__((target_version("ls64_v"))) int f54();
__attribute__((target_version("ls64_accdata"))) int f55();
__attribute__((target_version("wfxt"))) int f56();
__attribute__((target_version("sme-f64f64"))) int f57();
__attribute__((target_version("sme-i16i64"))) int f58();
__attribute__((target_version("sme2"))) int f59();
__attribute__((target_version("mops"))) int f60();
__attribute__((target_version("cssc"))) int f61();
__attribute__((target_version("neon"))) int f62();
__attribute__((target_version("crypto"))) int f63();
__attribute__((target_version("lse128"))) int f64();
__attribute__((target_version("sve2p1"))) int f65();
__attribute__((target_version("sme2p1"))) int f66();
__attribute__((target_version("fp8"))) int f67();
__attribute__((target_version("faminmax"))) int f68();
__attribute__((target_version("lut"))) int f69();
__attribute__((target_version("gcs"))) int f70();
__attribute__((target_version("mte"))) int f71();
__attribute__((target_version("rcpc-immo"))) int f72();
__attribute__((target_version("the"))) int f73();
__attribute__((target_version("d128"))) int f74();
__attribute__((target_version("v8.1a"))) int f75();
__attribute__((target_version("sha512"))) int f76();
__attribute__((target_version("sm3"))) int f77();
__attribute__((target_version("sme-f16f16"))) int f78();
__attribute__((target_version("sve2-aes+sve2-pmull128"))) int f79();
__attribute__((target_version("cpa"))) int f80();
