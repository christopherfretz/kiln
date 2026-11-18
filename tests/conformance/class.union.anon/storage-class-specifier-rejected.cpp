// [class.union.anon]/2: an anonymous union at class scope takes no
// storage-class-specifier; [dcl.stc]/9: `mutable` only on a non-static
// data member, so not on a block-scope anonymous union. Found running
// GCC's C++ testsuite (clang 22 and GCC 16.1 both reject).
//@ expect: compile-fail
//@ std: c++98
//@ strict-diags
//@ diag: 14:3 error "anonymous union at class scope must not have a storage specifier"
//@ diag: 17:3 error "anonymous union at class scope must not have a storage specifier"
//@ diag: 20:3 error "anonymous union at class scope must not have a storage specifier"
//@ diag: 23:3 error "anonymous union at class scope must not have a storage specifier"
//@ diag: 36:11 error "'mutable' can only be applied to member variables"
struct Gauge {
  static union {
    int raw;
  };
  extern union {
    float level;
  };
  mutable union {
    char tag;
  };
  register union {
    long wide;
  };
  union {
    short plain;
  };
};

static union {
  int shared_count;
};

int probe() {
  mutable union {
    int local;
  };
  static union {
    int kept;
  };
  return kept;
}
