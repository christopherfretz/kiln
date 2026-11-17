// [class.union.anon]/1, accepted side of
// member-specification-only-data-members.cpp: non-static data members of
// any kind (a bit-field, one with a default member initializer, one of an
// unnamed class type with a declarator, a pointer to a class first named
// by an elaborated-type-specifier, which declares that class in the
// enclosing scope), public members after an access specifier, a
// `static_assert`, an empty declaration, and an anonymous struct or union
// nested inside (the extension, -Wnested-anon-types: off by default). At
// namespace scope, at block scope, as a class member and in a class
// template. Found running GCC's C++ testsuite.
//@ expect: compile-ok
//@ std: c++26
static union {
  int a;
  int b : 3;
  struct { int x; } s;
  struct Fwd *p;
  public: long c;
  static_assert(sizeof(int) >= 2);
};
struct Fwd { int y; };
struct S {
  union {
    int d = 1;
    char e;
    struct { short f, g; };
    union { float h; unsigned i; };
  };
  struct { int j; union { int k; }; };
};
template <class T> struct TS {
  union { T a; struct { T b, c; }; };
};
TS<int> ts;
int f() {
  union { int x; struct { char y, z; }; };
  x = 1;
  Fwd w{2};
  S s;
  return x + w.y + s.d + s.f + s.j + ts.b + a + (p != nullptr);
}
