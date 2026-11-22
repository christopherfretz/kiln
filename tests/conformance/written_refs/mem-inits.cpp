// The written-reference table (spec/ast.md §18.4), role `mem-init`: the
// mem-initializer-id of each mem-initializer ([class.base.init]): a
// member, a direct base, a qualified base, a template-id base (role
// `mem-init` on the template name), a typedef of a base, a base named
// through a using-declaration, a delegating constructor, a template
// parameter base and a dependent base in a template.
//@ written-refs: mem-inits.refs
namespace lib {
struct Root {
  Root(int);
};
template <class T> struct Wrap {
  Wrap(T);
};
typedef Root RootAlias;
} // namespace lib

using lib::Root;

struct Member {
  int n;
  long m;
  Member(int v) : n(v), m{v} {}
  Member() : Member(0) {}            // delegating
};

struct Bases : Root, lib::Wrap<int> {
  int k;
  Bases(int v) : Root(v), lib::Wrap<int>(v), k(v) {}
  Bases() : lib::RootAlias(1), Wrap(2), k() {}
};

template <class T> struct Derived : T, lib::Wrap<T> {
  T t;
  Derived(T v) : T(v), lib::Wrap<T>(v), t(v) {}
};

template <class T> struct Dependent : T::base_type {
  Dependent() : T::base_type(0) {}
};
