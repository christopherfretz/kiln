export module lcctor;
// Local-class members with GNU `constructor` / `used` in a named module
// interface (WP-3687): the module's own object emits `g::L::c`, `f::L::c`
// and `fu::L::c` once; an importer must not emit duplicate copies or
// duplicate `global_ctors` entries, while instantiating exported template
// `t<int>()` in the importer emits `t<int>::L::c` there.
export int n_g = 0;
export int n_f = 0;
export int n_t = 0;

export void g() {
  struct L {
    [[gnu::constructor]] static void c() { ++n_g; }
  };
}

export inline void f() {
  struct L {
    [[gnu::constructor]] static void c() { ++n_f; }
  };
}

export inline void fu() {
  struct L {
    [[gnu::used]] static void c() {}
  };
}

export template <class T> void t() {
  struct L {
    [[gnu::constructor]] static void c() { ++n_t; }
  };
}
