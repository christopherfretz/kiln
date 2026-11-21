// Before C++26, clang accepts deleting a 'void *' with a warning
// (-Wdelete-incomplete, an extension: [expr.delete]/1 requires a pointer to
// an object type; C++26 makes it an error). clang's constant evaluator
// then deletes the object by its dynamic type: its destructor runs and its
// storage is released.
//@ std: c++23
//@ args: -Wno-delete-incomplete
constexpr int scalar() {
  void *p = new int(3);
  delete p;
  return 1;
}
static_assert(scalar() == 1);

constexpr int array() {
  void *p = new int[2];
  delete[] p;
  return 1;
}
static_assert(array() == 1);

struct S {
  int *c;
  constexpr ~S() { ++*c; }
};
constexpr int destroyed() {
  int c = 0;
  void *p = new S{&c};
  delete p;
  return c;
}
static_assert(destroyed() == 1);

// The dynamic type's own destructor: a virtual one from the most derived
// class, each element of a multidimensional array, a pointer reloaded
// from memory.
struct B {
  int *c;
  constexpr B(int *c) : c(c) {}
  constexpr virtual ~B() { *c += 100; }
};
struct D : B {
  constexpr D(int *c) : B(c) {}
  constexpr ~D() override { *c += 1; }
};
constexpr int derived() {
  int c = 0;
  void *p = new D(&c);
  delete p;
  return c;
}
static_assert(derived() == 101);

constexpr int multi() {
  int c = 0;
  void *p = new S[2][2]{{{&c}, {&c}}, {{&c}, {&c}}};
  delete[] p;
  return c;
}
static_assert(multi() == 4);

constexpr int empty() {
  void *p = new S[0];
  delete[] p;
  return 0;
}
static_assert(empty() == 0);

constexpr int reloaded() {
  int c = 0;
  void *v[1] = {new S{&c}};
  delete v[0];
  return c;
}
static_assert(reloaded() == 1);
