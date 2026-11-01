// [basic.stc.dynamic.general]/2 (WP-158 item g): the replaceable global
// allocation and deallocation functions are implicitly declared in the
// global scope, so `::operator new`/`operator delete[]` name them without
// `<new>` (also unqualified from a namespace or class, and in templates);
// taking the address of the overload set without a target type fails.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:7 error "variable 'fp' with type 'auto *' has incompatible initializer of type '<overloaded function type>'"
void *p = ::operator new(4);
void q() { ::operator delete(p); }
void *p2 = operator new[](8);
void q2() { operator delete[](p2); ::operator delete(p, 4ul); }
auto *fp = &::operator new;
void *(*fp2)(decltype(sizeof 0)) = ::operator new;
namespace ns { void *r() { return operator new(2); } }
struct C { void *m() { return operator new(3); } };
template <class T> T *mk() { return static_cast<T *>(::operator new(sizeof(T))); }
int *pi = mk<int>();
