// Declarations of img-attrs-more.cppm with attributes the declaration
// image carries since WP-1806.
typedef unsigned long iam_size_t;
extern "C" int iam_real(int x);
extern "C" void *iam_resolve();
extern "C" int iam_ov(int x) __attribute__((overloadable));
extern "C" int iam_ov(long x) __attribute__((overloadable));
namespace iam {
extern int cleaned;
inline void done(int *p) { cleaned += *p; }
inline int clean(int x) {
  int v __attribute__((cleanup(done))) = x;
  return v + 1;
}
inline iam_size_t osz(void *const p __attribute__((pass_object_size(0)))) {
  return __builtin_object_size(p, 0);
}
struct __attribute__((ms_struct)) MS {
  char a;
  int b : 4;
  char c;
};
int alias_of(int) __attribute__((alias("iam_real")));
int via_ifunc(int) __attribute__((ifunc("iam_resolve")));
extern int weak_one __attribute__((weak_import));
[[clang::annotate("iam_note", 3)]] inline int ann(int x) { return x + 9; }
__attribute__((section(".text.iam"))) inline int in_sec(int x) { return x + 1; }
} // namespace iam
