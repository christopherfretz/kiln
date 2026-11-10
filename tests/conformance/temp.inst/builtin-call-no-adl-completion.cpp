// [temp.inst]/2, [basic.lookup.argdep]/1: a call of an implicitly declared
// builtin with a type-dependent argument does not use argument-dependent
// lookup at instantiation (clang `UseArgumentDependentLookup`), so a
// pointer-to-specialization argument such as `Holder<Incomplete>*` does not
// implicitly instantiate `Holder<Incomplete>` (its completeness does not
// affect the call's semantics; instantiating it would be ill-formed). This
// is the libc++ `std::construct_at<Holder<Incomplete>*>` pattern
// (`std::addressof` -> `__builtin_addressof`).
//@ expect: run
//@ strict-diags
struct Incomplete;
template <class T> struct Holder { T t; };
namespace n {
template <class T> T* addr(T& x) { return __builtin_addressof(x); }
template <class T> T* launder(T* p) { return __builtin_launder(p); }
template <class T> void* voidify(T& x) {
  return const_cast<void*>(static_cast<const volatile void*>(n::addr(x)));
}
template <class T, class A> T* construct_at(T* p, A a) { return ::new (n::voidify(*p)) T(a); }
}  // namespace n
#include <new>
int main() {
  Holder<Incomplete>* p = nullptr;
  Holder<Incomplete>** q = n::addr(p);
  Holder<Incomplete>** r = n::launder(q);
  Holder<Incomplete>* slot = reinterpret_cast<Holder<Incomplete>*>(1);
  n::construct_at(&slot, nullptr);
  return (q == &p && r == q && slot == nullptr) ? 0 : 1;
}
