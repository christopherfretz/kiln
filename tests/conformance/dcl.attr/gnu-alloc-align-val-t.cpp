// A GNU alloc_align parameter index may name a parameter of type
// `std::align_val_t` as well as an integer parameter; alloc_size still
// needs an integer (WP-525; clang `AddAllocAlignAttr` / `Type::isAlignValT`
// vs `checkParamIsIntegerType`; libstdc++ 16 <new>
// declares `operator new(size_t, align_val_t)` with `__alloc_align__(2)`).
// Only an enumeration named `align_val_t` in namespace `std` qualifies,
// through typedefs; any other enumeration is still rejected. Verified
// against clang 19.
//@ expect: compile-fail
//@ diag: 22:61 error "'alloc_size' attribute argument may only refer to a function parameter of integer type"
//@ diag: 24:63 error "'alloc_align' attribute argument may only refer to a function parameter of integer type"
//@ diag: 25:58 error "'alloc_align' attribute argument may only refer to a function parameter of integer type"
using size_t = decltype(sizeof 0);
namespace std {
enum class align_val_t : size_t {};
inline namespace v1 { enum class other_t : size_t {}; }
}
namespace mine { enum class align_val_t : size_t {}; }
using al = std::align_val_t;
void* a(size_t, std::align_val_t) __attribute__((__alloc_align__(2)));
void* b(size_t, al) __attribute__((alloc_size(1), alloc_align(2)));
void* c(size_t, std::align_val_t) __attribute__((alloc_size(2)));
void* d(size_t, int) __attribute__((alloc_align(2)));
void* e(size_t, mine::align_val_t) __attribute__((alloc_align(2)));
void* f(size_t, std::other_t) __attribute__((alloc_align(2)));
