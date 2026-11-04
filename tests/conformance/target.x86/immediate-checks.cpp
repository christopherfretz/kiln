// [driver] clang's checks of x86 builtin immediates
// (`SemaX86::CheckBuiltinFunctionCall`; spec/landed/sema/WP-630.md
// "Immediates"): an integer constant expression is required; a range is
// `-Wargument-outside-range` (an error by default), except the AMX tile
// numbers' hard error; a rounding operand is 4, 8 or 9 to 11; a gather
// scale 1, 2, 4 or 8; tile operands are distinct; `readeflags_u32` is a
// 32-bit builtin. WP-630.
//@ args: --target=x86_64-unknown-linux-gnu
//@ expect: compile-fail
//@ strict-diags
//@ diag: 24:32 error "argument to '__builtin_ia32_pshufd' must be a constant integer"
//@ diag: 25:25 error "argument value 256 is outside the valid range [0, 255]"
//@ diag: 26:27 error "invalid rounding argument"
//@ diag: 28:36 error "invalid rounding argument"
//@ diag: 30:58 error "scale argument must be 1, 2, 4, or 8"
//@ diag: 31:12 error "tile arguments must refer to different tiles"
//@ diag: 32:12 error "argument value 9 is outside the valid range [0, 7]"
//@ diag: 33:23 error "this builtin is only available on 32-bit targets"
//@ diag: 34:30 error "argument value 2 is outside the valid range [0, 1]"
typedef int v4si __attribute__((vector_size(16)));
typedef float v16sf __attribute__((vector_size(64)));
typedef double v2df __attribute__((vector_size(16)));
typedef long long v2di __attribute__((vector_size(16)));
v4si a(v4si x, int n) { return __builtin_ia32_pshufd(x, n); }
v4si b(v4si x) { return __builtin_ia32_pshufd(x, 256); }
v16sf c(v16sf x) { return __builtin_ia32_addps512(x, x, 5); }
v16sf d(v16sf x) { return __builtin_ia32_addps512(x, x, 9); }
unsigned short e(v16sf x) { return __builtin_ia32_cmpps512_mask(x, x, 1, 1, 9); }
unsigned short ok(v16sf x) { return __builtin_ia32_cmpps512_mask(x, x, 31, 1, 12); }
v2df f(v2df s, const double *p, v4si i, v2df m) { return __builtin_ia32_gatherd_pd(s, p, i, m, 3); }
void g() { __builtin_ia32_tdpbssd(1, 1, 2); }
void h() { __builtin_ia32_tilezero(9); }
unsigned k() { return __builtin_ia32_readeflags_u32(); }
long long l(v2di x) { return __builtin_ia32_vec_ext_v2di(x, 2); }
