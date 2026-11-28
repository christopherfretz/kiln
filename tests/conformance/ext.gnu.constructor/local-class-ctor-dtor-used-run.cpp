// GNU `constructor` / `destructor` / `used` / `retain` on member functions of
// local classes (reduced from real code, WP-3687): local-class member
// functions carrying a forced-emission attribute are emitted even when their
// enclosing function is unused, inline, constexpr, consteval, a member
// function, a lambda only constant-evaluated, or a template instantiation;
// uninstantiated template patterns are not emitted, and calling a
// `constructor`-marked method does not register a duplicate global_ctors entry.
//@ expect: run
//@ std: c++20
//@ stdout: ok
//@ stdout: dtor-g
extern "C" int puts(const char *);

int ran_g = 0;
int ran_gn = 0;
int ran_nf = 0;
int ran_called = 0;
int ran_s = 0;
int ran_t = 0;
int ran_it = 0;
int ran_uninst = 0;
int ran_fc = 0;
int ran_fce = 0;
int ran_k = 0;
int ran_gl = 0;
int ran_gl_uninst = 0;
int ran_a = 0;

void g() {
  struct L {
    __attribute__((constructor)) static void c() { ++ran_g; }
  };
}

void gn() {
  struct L {
    struct M {
      __attribute__((constructor)) static void c() { ++ran_gn; }
    };
  };
}

void nested_fn() {
  struct L {
    static void m() {
      struct M {
        __attribute__((constructor)) static void c() { ++ran_nf; }
      };
    }
  };
}

void called_ctor() {
  struct L {
    __attribute__((constructor)) static void c() { ++ran_called; }
  };
  L::c();
}

void gd() {
  struct L {
    __attribute__((destructor)) static void d() { puts("dtor-g"); }
  };
}

void gu() {
  struct L {
    [[gnu::used]] static void c() {}
    __attribute__((used, retain)) static void cr() {}
    __attribute__((used)) void m() {}
  };
}

inline void f() {
  struct L {
    __attribute__((used)) static void c() {}
    __attribute__((used)) void m() {}
  };
}

struct S {
  void mf() {
    struct L {
      __attribute__((constructor)) static void c() { ++ran_s; }
      __attribute__((used)) L() {}
      __attribute__((used)) ~L() {}
    };
  }
};

template <class T> void t() {
  struct L {
    __attribute__((constructor)) static void c() { ++ran_t; }
  };
}
template void t<int>();

template <class T> void it() {
  struct L {
    __attribute__((constructor)) static void c() { ++ran_it; }
  };
}

template <class T> void uninst() {
  struct L {
    __attribute__((constructor)) static void c() { ++ran_uninst; }
  };
}

constexpr int fc() {
  struct L {
    __attribute__((constructor)) static void c() { ++ran_fc; }
  };
  return 1;
}
static_assert(fc() == 1);

consteval int fce() {
  struct L {
    __attribute__((constructor)) static void c() { ++ran_fce; }
  };
  return 1;
}
static_assert(fce() == 1);

constexpr int k = [] {
  struct L {
    __attribute__((constructor)) static void c() { ++ran_k; }
  };
  return 1;
}();

auto gl = [](auto x) {
  struct L {
    __attribute__((constructor)) static void c() { ++ran_gl; }
  };
  return x;
};

auto gl_uninst = [](auto x) {
  struct L {
    __attribute__((constructor)) static void c() { ++ran_gl_uninst; }
  };
  return x;
};

template <class T> struct A {
  static constexpr int k = [] {
    struct L {
      __attribute__((constructor)) static void c() { ++ran_a; }
    };
    return 1;
  }();
};

int main() {
  (void)&A<int>::k;
  called_ctor();
  it<int>();
  (void)gl(0);
  if (ran_g != 1 || ran_gn != 1 || ran_nf != 1 || ran_called != 2 || ran_s != 1 || ran_t != 1 ||
      ran_it != 1 || ran_uninst != 0 || ran_fc != 1 || ran_fce != 1 || ran_k != 1 || ran_gl != 1 ||
      ran_gl_uninst != 0 || ran_a != 1) {
    return 1;
  }
  puts("ok");
  return 0;
}
