// spec/reflection.md §3.5: a result class the Oven cannot build (here a
// string_view without the (pointer, size) layout) is diagnosed at the
// call, naming the class and the function.
//@ expect: compile-fail
//@ args: -freflection
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ diag: 22:15 error "static assertion expression is not an integral constant expression"
//@ diag: 22:39 note "'string_view' cannot be used by 'std::meta::identifier_of': it does not hold a 'const char *'"
namespace std {
template <class C>
struct basic_string_view {
  int size_;
};
using string_view = basic_string_view<char>;
namespace meta {
using info = decltype(^^int);
consteval string_view identifier_of(info r);
}  // namespace meta
}  // namespace std
struct S {};
static_assert(std::meta::identifier_of(^^S).size_ == 1);

int main() {}
