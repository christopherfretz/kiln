// The other unit of ../lambda-in-signature-explicit-args-conversions-two-units-run.cpp:
// it calls the `long` specializations' closures under the names clang 19
// and 22 give them (asm labels), so the program links only if kiln defines
// the same names. A closure's call operator takes its object pointer first.
int fs_op(const void *) __asm__("_ZZ2fsT_ENKUlvE3_clEv"); // static_cast<FP>
int fc_op(const void *) __asm__("_ZZ2fcT_ENKUlvE3_clEv"); // (FP)
int fr_op(const void *) __asm__("_ZZ2frT_ENKUlvE2_clEv"); // static_cast<FR>
int fg_op(const void *) __asm__("_ZZ2fgT_ENKUlvE3_clEv"); // ::take(fg<int>)
int fh_op(const void *) __asm__("_ZZ2fhT_ENKUlvE3_clEv"); // takeref(fh<int>)
int fd_op(const void *) __asm__("_ZZ2fdT_ENKUlvE5_clEv"); // deduce(fd<int>)
int fl_op(const void *) __asm__("_ZZ2flT_ENKUlvE4_clEv"); // FP l{fl<int>}
int fk_op(const void *) __asm__("_ZZ2fkT_ENKUlvE4_clEv"); // S k{fk<int>}
int fu_op(const void *) __asm__("_ZZ2fuT_ENKUlvE4_clEv"); // auto *u
int fv_op(const void *) __asm__("_ZZ2fvT_ENKUlvE3_clEv"); // auto &v
int fn_op(const void *) __asm__("_ZZ2fnT_ENKUlvE4_clEv"); // TA<&fn<int>>
int other_conversions() {
  return fs_op(nullptr) + fc_op(nullptr) + fr_op(nullptr) + fg_op(nullptr) + fh_op(nullptr) +
         fd_op(nullptr) + fl_op(nullptr) + fk_op(nullptr) + fu_op(nullptr) + fv_op(nullptr) +
         fn_op(nullptr);
}
