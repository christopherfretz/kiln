// [basic.lookup.qual.general]/2-4, [expr.prim.id.dtor]/2: in
// `p->N::I::~B<T>()` on a dependent object the member-qualified `N` is
// found by unqualified lookup at instantiation (2) and `B` in the prefix
// `N` (4.2); with `T = int` both type-names name `int`, so the call is a
// pseudo-destructor call that still evaluates its object expression.
// `N` is dependent at the definition ([temp.dep.type]/5.2) and, in the
// specialization, not ([basic.lookup.qual.general]/3 note 2): `int` is no
// class, so `N` undergoes unqualified lookup (/3, last sentence), exactly
// as in the non-dependent `p->N::I::~B<int>()` every compiler accepts.
// clang 19/22 ("use of undeclared identifier 'N'") and GCC 16 ("'N::I' is
// not a class member", only with the template-id `~B<T>`) both reject it;
// kiln follows the standard (WP-1155, WP-1173; spec/sema-class.md).
//@ expect: run
//@ strict-diags
//@ oracle: none
//@ xfail(clang): clang 19/22 do not look `N` up unqualified at instantiation (WP-1173)
//@ xfail(gcc): GCC 16 rejects `p->N::I::~B<T>()` on a dependent object (WP-1173)
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
