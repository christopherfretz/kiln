// Local-class member functions with GNU `constructor` / `destructor` / `used` /
// `retain` (reduced from real code, WP-3687): emitted with clang's linkage
// (`internal` for non-inline functions and internal-linkage lambdas;
// `linkonce_odr` for inline/constexpr/consteval functions, member functions,
// template instantiations and external-linkage lambdas) without emitting unused
// enclosing functions; uninstantiated template patterns and bare `retain` stay
// unemitted.
extern "C" int puts(const char *);

void g() {
  struct L {
    __attribute__((constructor)) static void c() { puts("ran-g"); }
  };
}

void called_ctor() {
  struct L {
    __attribute__((constructor)) static void c() { puts("called"); }
  };
  L::c();
}

void nested_fn() {
  struct L {
    static void m() {
      struct M {
        __attribute__((constructor)) static void c() { puts("nested-fn"); }
      };
    }
  };
}

void gd() {
  struct L {
    __attribute__((destructor)) static void d() { puts("dtor-g"); }
  };
}

void gu() {
  struct L {
    [[gnu::used]] static void c() { puts("used-g"); }
  };
}

void gur() {
  struct L {
    __attribute__((used, retain)) static void c() { puts("retain-g"); }
  };
}

void gr() {
  struct L {
    __attribute__((retain)) static void c() { puts("bare-retain"); }
  };
}

void gns() {
  struct L {
    __attribute__((used)) void m() { puts("nonstatic-g"); }
  };
}

inline void f() {
  struct L {
    __attribute__((used)) static void c() { puts("ran-f"); }
  };
}

inline void fns() {
  struct L {
    __attribute__((used)) void m() { puts("nonstatic-f"); }
  };
}

struct S {
  void mf() {
    struct L {
      __attribute__((constructor)) static void c() { puts("mf"); }
      __attribute__((used)) L() { puts("ctor"); }
      __attribute__((used)) ~L() { puts("dtor"); }
    };
  }
};

template <class T> void t() {
  struct L {
    __attribute__((constructor)) static void c() { puts("ran-t"); }
  };
}
template void t<int>();

template <class T> void it() {
  struct L {
    __attribute__((constructor)) static void c() { puts("ran-it"); }
  };
}

template <class T> void uninst() {
  struct L {
    __attribute__((constructor)) static void c() { puts("uninst"); }
  };
}

constexpr int fc() {
  struct L {
    __attribute__((constructor)) static void c() { puts("ran-fc"); }
  };
  return 1;
}
static_assert(fc() == 1);

consteval int fce() {
  struct L {
    __attribute__((constructor)) static void c() { puts("ran-fce"); }
  };
  return 1;
}
static_assert(fce() == 1);

constexpr int k = [] {
  struct L {
    __attribute__((constructor)) static void c() { puts("ran-k"); }
  };
  return 1;
}();

auto gl = [](auto x) {
  struct L {
    __attribute__((constructor)) static void c() { puts("ran-gl"); }
  };
  return x;
};

auto gl_uninst = [](auto x) {
  struct L {
    __attribute__((constructor)) static void c() { puts("gl-uninst"); }
  };
  return x;
};

template <class T> struct A {
  static constexpr int k = [] {
    struct L {
      __attribute__((constructor)) static void c() { puts("ran-A"); }
    };
    return 1;
  }();
};

int main() {
  (void)&A<int>::k;
  called_ctor();
  it<int>();
  return gl(0);
}
