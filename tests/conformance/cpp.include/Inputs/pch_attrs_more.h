// Declarations of pch-attrs-more-run.cpp with attributes a precompiled
// header's snapshot carries (WP-1806).
extern "C" int pam_real(int x);
extern "C" void *pam_resolve();
extern "C" int pam_ov(int x) __attribute__((overloadable));
extern "C" int pam_ov(long x) __attribute__((overloadable));
namespace pam {
inline int cleaned = 0;
inline void done(int *p) { cleaned += *p; }
inline int clean(int x) {
  int v __attribute__((cleanup(done))) = x;
  return v + 1;
}
inline unsigned long osz(void *const p __attribute__((pass_object_size(0)))) {
  return __builtin_object_size(p, 0);
}
struct __attribute__((ms_struct)) MS {
  char a;
  int b : 4;
  char c;
};
int alias_of(int) __attribute__((alias("pam_real")));
int via_ifunc(int) __attribute__((ifunc("pam_resolve")));
extern int weak_one __attribute__((weak_import));
[[clang::annotate("pam_note", 3)]] inline int ann(int x) { return x + 9; }
__attribute__((section(".text.pam"))) inline int in_sec(int x) { return x + 1; }
} // namespace pam
