// Interface unit of module `cl` for carried-literals-run.cpp
// (spec/modules.md §11.11 Part 2, WP-1160): constexpr pointers to string
// literals (one in a template's body, which every specialization shares,
// and `__func__`), to lifetime-extended temporaries (bound by a
// reference and by a reference member) and to labels, all evaluated
// here, so a kind-2 module file carries them.
export module cl;
export constexpr const char *s1 = "abc";
export constexpr const char *s2 = &"abcd"[2];
export constexpr const char *ps = s1;
export constexpr const char8_t *s8 = u8"xy";
export constexpr const int &t1 = 42;
export struct P {
  int a, b;
};
export constexpr const P &tp = P{3, 4};
export constexpr const int *tb = &tp.b;
export struct R {
  const int &r;
};
export constexpr R rr{9};
template <class T> constexpr const char *name() { return "tmpl"; }
export constexpr const char *n1 = name<int>();
export constexpr const char *n2 = name<long>();
export inline const char *fu() {
  static constexpr const char *p = __func__;
  return p;
}
export inline int lab(int i) {
  static constexpr void *la = &&a;
  static constexpr void *lb = &&b;
  goto *(i ? lb : la);
a:
  return 1;
b:
  return 2;
}
