// [class.temporary]/3 and clang's `__attribute__((trivial_abi))` (AttrDocs
// "trivial_abi"): a class with the attribute is passed and returned as if
// trivially copyable (in registers, the callee destroying parameters), so a
// returned local is a different object from the caller's even with a
// non-trivial move constructor and destructor (no NRVO into the caller's
// slot). libc++ 22's unique_ptr / shared_ptr / weak_ptr use it under
// `_LIBCPP_ABI_ENABLE_*_TRIVIAL_ABI`; libcxx/memory/trivial_abi/*.pass
// assert the address change. kiln ignores the attribute ("unknown attribute
// 'trivial_abi' ignored") and returns through the caller's slot, so main
// returns 1: the M5 cluster "trivial_abi" (docs/status/libcxx-m5.md).
//@ expect: run
//@ xfail(kiln): the trivial_abi attribute is not implemented (attr.rs, codegen argument/return passing)
struct __attribute__((trivial_abi)) P { int* p; P(int* q) : p(q) {} P(P&& o) : p(o.p) { o.p = nullptr; } ~P() {} };
template <class T> struct __attribute__((trivial_abi)) TP { T* p; TP(T* q) : p(q) {} TP(TP&& o) : p(o.p) { o.p = nullptr; } ~TP() {} };
__attribute__((noinline)) P make(void** a) { P r(nullptr); *a = &r; return r; }
__attribute__((noinline)) TP<int> make2(void** a) { TP<int> r(nullptr); *a = &r; return r; }
int main() { void* a = nullptr; P r = make(&a); if ((void*)&r == a) return 1; TP<int> r2 = make2(&a); if ((void*)&r2 == a) return 2; return 0; }
