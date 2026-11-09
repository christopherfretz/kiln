// [basic.lookup.qual.general]/2-4, [expr.prim.id.dtor]/2: in
// `p->N::I::~B<T>()` on a dependent object the member-qualified `N` is
// found by unqualified lookup at instantiation (2) and `B` in the prefix
// `N` (4.2); with `T = int` both type-names name `int`, so the call is a
// pseudo-destructor call that still evaluates its object expression.
// clang 19/22 ("use of undeclared identifier 'N'") and GCC 16 ("'N::I' is
// not a class member") both reject it; kiln follows the standard.
//@ expect: run
//@ strict-diags
//@ skip(clang): clang 19/22 do not look `N` up unqualified on a dependent object (WP-1155)
//@ skip(gcc): GCC 16 rejects a namespace-qualified pseudo-destructor on a dependent object (WP-1155)
//@ stdout: calls=2
extern "C" int printf(const char*, ...);
namespace N {
using I = int;
template <class T> using B = T;
}
int calls;
template <class T> T* obj(T* p) { ++calls; return p; }
template <class T> void prefix(T* p) {
  obj(p)->N::I::~B<T>();
  obj(p)->::N::I::~I();
}
int main() {
  int i = 0;
  prefix(&i);
  printf("calls=%d\n", calls);
}
