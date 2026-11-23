// The found set (spec/ast.md §18.12) of a dependent call's name with
// three visible overloads ([basic.lookup.unqual], [temp.dep.candidate]/1):
// one declared in the scope, one through a using-declaration (the
// element's `found` is the using-shadow, [namespace.udecl]/1) and one
// through a using-directive (the element names it, [namespace.udir]/2).
// Each specialization's row selects one of them.
//@ written-refs: found-several.refs
namespace n { void f(long); }
namespace m { void f(char); }
void f(int);
using n::f;
using namespace m;
template <class T> void several(T t) { f(t); }
void use() {
  several(1);
  several(2L);
  several('c');
}
