// clang `weak_import` (`WeakImportAttr`, `Decl::isWeakImported`): a
// variable or function declared with it and not defined in the
// translation unit is an `extern_weak` reference, including through a
// redeclaration, a block-scope `extern`, a static data member, a
// function or variable template specialization and an alias; a
// definition (an earlier declaration's attribute notwithstanding) is
// emitted as usual. spec/sema.md §4.5, WP-1745.
//@ expect: ir
extern int a __attribute__((weak_import));
extern int c;
extern int c __attribute__((weak_import));
void f() __attribute__((weak_import));
void h();
[[clang::weak_import]] void h();
extern int d __attribute__((weak_import));
int d = 3;
void i() __attribute__((weak_import));
void i() {}
struct S { static int m __attribute__((weak_import)); };
extern "C" void ec() __attribute__((weak_import));
template <class X> void tf() __attribute__((weak_import));
template <class X> extern int tv __attribute__((weak_import));
extern "C" void target() {}
extern "C" void al() __attribute__((alias("target"), weak_import));
int use() {
  extern int le __attribute__((weak_import));
  f(); h(); i(); ec(); al(); tf<int>();
  return a + c + d + S::m + le + tv<int> + (&a != nullptr);
}
