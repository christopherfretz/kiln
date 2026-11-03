// [meta.reflection.synop], [namespace.def]/6 (spec/reflection.md §3.1): an
// intrinsic is recognized by its name and signature in namespace
// `std::meta`, whatever inline namespaces a standard library wraps it in
// (`std::__1` here, as libc++ does). A same-named function with another
// signature, or the same declaration outside `std::meta`, is an ordinary
// (here undefined) function: calling it is not a constant expression.
//@ expect: compile-fail
//@ args: -freflection
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ diag: 36:15 error "static assertion expression is not an integral constant expression"
//@ diag: 36:15 note "undefined function 'type_of' cannot be used in a constant expression"
//@ diag: 23:16 note "declared here"
//@ diag: 37:15 error "static assertion expression is not an integral constant expression"
//@ diag: 37:15 note "undefined function 'type_of' cannot be used in a constant expression"
//@ diag: 27:22 note "declared here"
namespace std {
inline namespace __1 {
namespace meta {
using info = decltype(^^int);
consteval info type_of(info r);
consteval const char* name_of(info r);
consteval info type_of(int r);
}  // namespace meta
}  // namespace __1
namespace other {
consteval meta::info type_of(meta::info r);
}
}  // namespace std

int v;
struct S {};
static_assert(std::meta::type_of(^^v) == ^^int);
static_assert(std::meta::name_of(^^S)[0] == 'S');
static_assert(std::meta::name_of(^^v)[1] == '\0');
static_assert(std::meta::type_of(0) == ^^int);
static_assert(std::other::type_of(^^v) == ^^int);
