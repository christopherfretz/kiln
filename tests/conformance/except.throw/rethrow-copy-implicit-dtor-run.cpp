// [except.throw]/5: the destructor of a thrown class object is
// potentially invoked, so the throw-expression odr-uses it and an
// implicit one is defined in this translation unit (clang
// `CheckCXXThrowOperand` `MarkFunctionReferenced`). Here `throw *p;` and
// `throw ex;` copy lvalues of a class whose implicit destructor nothing
// else uses (the original object is never destroyed), yet `__cxa_throw`
// is passed it. kiln left it undefined: a link error. WP-799.
//@ expect: run
//@ strict-diags
//@ stdout: inner 7|outer 7|dtors 2
#include <new>

extern "C" int printf(const char *, ...);

static int dtors = 0;

struct Count {
  ~Count() { ++dtors; }
};

struct Base {
  virtual int code() const { return 0; }
};

struct Err : Base {
  int c;
  Count n;
  explicit Err(int c) : c(c) {}
  int code() const override { return c; }
};

alignas(Err) static unsigned char buf[sizeof(Err)];

[[gnu::noinline]] void raise(int c) {
  Err *p = ::new (buf) Err(c);
  throw *p;
}

[[gnu::noinline]] void relay() {
  try {
    raise(7);
  } catch (Err &ex) {
    printf("inner %d|", ex.code());
    throw ex;
  }
}

int main() {
  try {
    relay();
  } catch (Base &b) {
    printf("outer %d|", b.code());
  }
  printf("dtors %d\n", dtors);
}
