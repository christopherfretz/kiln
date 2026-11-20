// Before C++26, clang accepts deleting a 'void *' with a warning
// (-Wdelete-incomplete, an extension: [expr.delete]/1 requires a pointer to
// an object type; C++26 makes it an error). clang's constant evaluator
// then deletes the object by its dynamic type: its destructor runs and its
// storage is released.
//@ std: c++23
//@ args: -Wno-delete-incomplete
//@ xfail(kiln): WP-1741 remainder: delete through 'void *' needs the destructor of the allocation's dynamic type (compile/new_delete.rs delete_expr)
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
