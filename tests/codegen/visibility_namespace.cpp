//@ args: -std=c++26
// Namespace visibility (spec/codegen.md "Symbol visibility"; clang
// getLVForNamespaceScopeDecl): a `visibility` attribute on any
// definition of a namespace applies to every entity declared in it,
// including in a reopened definition without the attribute and in
// nested namespaces; an attribute on the entity, or a nested
// namespace's own attribute, overrides it.
namespace __attribute__((visibility("hidden"))) N {
void a() {}
int v = 1;
} // namespace N
namespace N {
void b() {}
__attribute__((visibility("default"))) void c() {}
namespace Inner {
void d() {}
}
namespace __attribute__((visibility("default"))) Out {
void e() {}
} // namespace Out
struct S {
  void m();
  virtual ~S();
};
void S::m() {}
S::~S() {}
} // namespace N
namespace O {
namespace __attribute__((visibility("hidden"))) I {
void f() {}
}
} // namespace O
namespace O::I {
void g() {}
}
int use() {
  N::a();
  N::S s;
  s.m();
  return N::v;
}
