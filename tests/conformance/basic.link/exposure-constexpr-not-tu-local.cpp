// [basic.link]/15-17: none of these is an exposure. A `const` variable
// that is not constexpr has its initializer ignored (/15.2); a TU-local
// constexpr variable (static, in an unnamed namespace) may hold a
// TU-local value; a pointer to an external variable, a null pointer,
// and a non-inline function template whose body names a TU-local
// function (/15.1) are not TU-local; and in the private module fragment
// the rule does not apply (/17: such a declaration elsewhere is
// deprecated, not ill-formed; GCC 16.1 rejects `pp`). clang 22 and GCC
// 16.1 accept the rest (WP-1669).
//@ expect: compile-ok
//@ std: c++20
//@ strict-diags
export module M;
static int hidden;
static int helper() { return 1; }
int ext;
export const int *const cp = &hidden;
export int *const q = &hidden;
static constexpr int *sp = &hidden;
namespace {
constexpr int *ap = &hidden;
}
export constexpr int *pe = &ext;
export constexpr int *pn = nullptr;
export template <class T> int t() { return helper(); }
export int use() { return *sp + *ap + *cp + *q; }
module :private;
constexpr int *pp = &hidden;
