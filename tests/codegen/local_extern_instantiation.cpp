// [basic.link]/6: a block-scope function or `extern` variable
// declaration names the namespace-scope entity (clang
// `mangleNameWithAbiTags` walks a local container out to its
// namespace): `_Z1gi` and `@ev`, not `_ZZ1fIiEvvE1gi`, in a plain
// function and in an instantiation, also under a namespace.
template <class T> void f() { extern void g(int); extern int ev; g(ev); }
void plain() { extern void p(long); extern int pv; p(pv); }
namespace N {
void g(int);
template <class T> void f2() { extern void g(int); extern int nv; g(nv); }
}
void h() { f<int>(); f<char>(); N::f2<char>(); }
