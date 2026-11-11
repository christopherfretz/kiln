// [dcl.link]/2, [except.spec]/2, [namespace.def]/5 (inline namespaces), [namespace.unnamed]/1,
// [lex.name]/3 (reserved names), [basic.stc.dynamic.general]/2, [dcl.typedef]/9,
// spec/modules.md §11.11 "Scopes and reserved names" (WP-1278): a module of
// linkage specifications, inline and unnamed namespaces, reserved names in
// the namespace std, builtin functions and unnamed types named by typedefs.
// Under `--bmi-image` the importer builds `imgs` from the image; the
// program behaves exactly as with the replayed source.
//@ expect: run
//@ std: c++20
//@ module: imgs=Inputs/img-scopes.cppm
//@ stdout: 5 6 2 2 1 20 22 103 8 5 5 6 18 4 31 9 9 8
import imgs;
extern "C" int printf(const char *, ...);
static_assert(lib::version() == 2 && lib::v2::version() == 2 && lib::v1::version() == 1);
static_assert(std::__twice(4) == 8 && std::__pair_like{2, 3}.__sum() == 5);
static_assert(std::__imgs::__twice(1) == 2);
static_assert(noexcept(imgs_nt(1)) && !noexcept(imgs_nf(1)) && noexcept(imgs_na(1)));
static_assert(imgs_len("abcd") == 4 && __builtin_strlen("ab") == 2);

int main() {
  int *p = new int(6);
  int q = *p;
  delete p;
  printf("%d %d %d %d %d %d %d %d %d %d %d %d %d %lu %d %d %d %d\n", imgs_c_add(2, 3),
         imgs_c_mul(2, 3), imgs_cxx(3),
         lib::version(), lib::v1::version(), lib::Tag{}.v, use_helper(7), __imgs_global(3),
         std::__twice(4), std::__pair_like{2, 3}.__sum(), heap_val(), q,
         imgs_nt(1) + imgs_nf(1) + imgs_na(1), imgs_len("wxyz"), imgs_clz(1u),
         imgs_abs(-9) + __builtin_abs(0), imgs_pod_sum(imgs_pod{4, 5}),
         imgs_hue(imgs_color(4)));
}
