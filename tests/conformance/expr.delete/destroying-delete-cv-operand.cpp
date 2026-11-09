// [expr.delete]/7.1: a destroying operator delete is passed the operand
// converted to its first parameter's type `C *`; the operand's
// cv-qualifiers do not matter (only access and ambiguity are checked;
// clang `ActOnCXXDelete`), so deleting a `const volatile C *` is valid
// (WP-1040).
//@ std: c++26
//@ expect: compile-ok
//@ strict-diags
namespace std {
using size_t = decltype(sizeof(0));
enum class align_val_t : size_t;
struct destroying_delete_t { explicit destroying_delete_t() = default; };
inline constexpr destroying_delete_t destroying_delete{};
}
using dd_t = std::destroying_delete_t;
struct A { void operator delete(A *, dd_t); };
struct B : A {};
void f(const volatile A *a, const B *b) {
  delete a;
  delete b;
}
