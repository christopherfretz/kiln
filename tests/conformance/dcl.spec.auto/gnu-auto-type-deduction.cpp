// [dcl.type.auto.deduct]/4: GNU '__auto_type' deduces as 'auto' does, from
// C++11 on, for variables, references, pointers, for-range and condition
// declarations and non-type template parameters (C++17); -Wgnu-auto-type
// (under -pedantic) names the extension (clang `ext_auto_type`; WP-1134).
//@ std: c++17
//@ args: -Wgnu-auto-type
//@ strict-diags
//@ diag: 16:3 warning "'__auto_type' is a GNU extension"
//@ diag: 18:3 warning "'__auto_type' is a GNU extension"
//@ diag: 21:3 warning "'__auto_type' is a GNU extension"
//@ diag: 23:3 warning "'__auto_type' is a GNU extension"
//@ diag: 25:8 warning "'__auto_type' is a GNU extension"
//@ diag: 26:7 warning "'__auto_type' is a GNU extension"
//@ diag: 28:10 warning "'__auto_type' is a GNU extension"
void g(int (&arr)[2]) {
  __auto_type d{1};
  static_assert(__is_same(decltype(d), int), "");
  __auto_type e = static_cast<const int>(1);
  static_assert(__is_same(decltype(e), int), "");
  int a = 0;
  __auto_type &&f = a;
  static_assert(__is_same(decltype(f), int &), "");
  __auto_type *p = &a;
  static_assert(__is_same(decltype(p), int *), "");
  for (__auto_type &v : arr) v = 1;
  if (__auto_type q = 1) (void)q;
}
template<__auto_type N> struct NT { static_assert(__is_same(decltype(N), int), ""); };
NT<1> nt;
