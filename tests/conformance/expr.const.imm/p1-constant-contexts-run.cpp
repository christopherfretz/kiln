// [expr.const.imm]/1.2 (P4101R1): a subexpression of a manifestly
// constant-evaluated expression is in an immediate function context, so
// a call of a consteval function returning a consteval-only value (a
// reflection, or a class holding one) is allowed there: an explicit
// function-template argument, a class-template argument (also in a
// base-specifier and an alias template), a variable-template argument, a
// default template argument, an array bound, a `static_assert`, a
// requires-clause, a `noexcept` operand, a `case` label, a `constexpr`
// local then used as a template argument, a pack of arguments, a member
// function, a lambda and a nested call. Non-dependent spelling; the
// instantiated one is p1-constant-contexts-dependent-run.cpp. Compared
// with GCC 16.1 (WP-1328).
//@ expect: run
//@ oracle: gcc
//@ args: -freflection
//@ skip(clang): clang 19/22 do not implement P2996 reflection
//@ strict-diags
//@ stdout: ok
extern "C" int puts(const char*);
using info = decltype(^^int);
struct Box { info r; int n; };
template <class T> consteval info key() { return ^^T; }
template <class T> consteval Box box() { return {^^T, 1}; }
consteval int arity(info r) { return r == ^^int ? 1 : 2; }
consteval int arity_b(Box b) { return b.r == ^^int ? b.n : 2; }
template <info H> void use() {}
template <Box B> void useb() {}
template <info... Hs> void usepk() {}
template <Box... Bs> void usepkb() {}
template <info H> struct C { static constexpr int v = 1; };
template <Box B> struct CB { static constexpr int v = 1; };
template <info H> constexpr int var = 1;
template <Box B> constexpr int varb = 1;
template <class... Rs> constexpr info vk = ^^int;
template <class... Rs> constexpr Box vkb = {^^int, 1};
template <class... Rs> consteval info key_of() { return vk<Rs...>; }
template <class... Rs> consteval Box box_of() { return vkb<Rs...>; }
namespace c_fn_arg_r {
void t() { use<key<int>()>(); }
int go() { t(); return 0; }
} // namespace c_fn_arg_r
namespace c_fn_arg_b {
void t() { useb<box<int>()>(); }
int go() { t(); return 0; }
} // namespace c_fn_arg_b
namespace c_fn_arg_var_pack_r {
void t() { use<key_of<int, long>()>(); }
int go() { t(); return 0; }
} // namespace c_fn_arg_var_pack_r
namespace c_fn_arg_var_pack_b {
void t() { useb<box_of<int, long>()>(); }
int go() { t(); return 0; }
} // namespace c_fn_arg_var_pack_b
namespace c_fn_arg_vartmpl_r {
void t() { use<vk<int>>(); }
int go() { t(); return 0; }
} // namespace c_fn_arg_vartmpl_r
namespace c_fn_arg_vartmpl_b {
void t() { useb<vkb<int>>(); }
int go() { t(); return 0; }
} // namespace c_fn_arg_vartmpl_b
namespace c_class_arg_r {
int t() { return C<key<int>()>::v; }
int go() { return t() - 1; }
} // namespace c_class_arg_r
namespace c_class_arg_b {
int t() { return CB<box<int>()>::v; }
int go() { return t() - 1; }
} // namespace c_class_arg_b
namespace c_base_spec_r {
struct D : C<key<int>()> {};
int go() { return D::v - 1; }
} // namespace c_base_spec_r
namespace c_base_spec_b {
struct D : CB<box<int>()> {};
int go() { return D::v - 1; }
} // namespace c_base_spec_b
namespace c_alias_r {
using A = C<key<int>()>;
int go() { return A::v - 1; }
} // namespace c_alias_r
namespace c_alias_b {
using A = CB<box<int>()>;
int go() { return A::v - 1; }
} // namespace c_alias_b
namespace c_var_arg_r {
int t() { return var<key<int>()>; }
int go() { return t() - 1; }
} // namespace c_var_arg_r
namespace c_var_arg_b {
int t() { return varb<box<int>()>; }
int go() { return t() - 1; }
} // namespace c_var_arg_b
namespace c_default_fn_r {
template <info H = key<int>()> void d() {}
int go() { d(); return 0; }
} // namespace c_default_fn_r
namespace c_default_fn_b {
template <Box H = box<int>()> void d() {}
int go() { d(); return 0; }
} // namespace c_default_fn_b
namespace c_default_class_r {
template <info H = key<int>()> struct DC {};
int go() { DC<> x; (void)x; return 0; }
} // namespace c_default_class_r
namespace c_default_class_b {
template <Box H = box<int>()> struct DCB {};
int go() { DCB<> x; (void)x; return 0; }
} // namespace c_default_class_b
namespace c_array_bound_r {
int t() { int a[arity(key<int>())] = {}; return sizeof a; }
int go() { return t() == 4 ? 0 : 1; }
} // namespace c_array_bound_r
namespace c_array_bound_b {
int t() { int a[arity_b(box<int>())] = {}; return sizeof a; }
int go() { return t() == 4 ? 0 : 1; }
} // namespace c_array_bound_b
namespace c_static_assert_r {
void t() { static_assert(arity(key<int>()) == 1); }
int go() { t(); return 0; }
} // namespace c_static_assert_r
namespace c_static_assert_b {
void t() { static_assert(arity_b(box<int>()) == 1); }
int go() { t(); return 0; }
} // namespace c_static_assert_b
namespace c_requires_r {
template <class U> requires (arity(key<int>()) == 1) void t() {}
int go() { t<int>(); return 0; }
} // namespace c_requires_r
namespace c_requires_b {
template <class U> requires (arity_b(box<int>()) == 1) void t() {}
int go() { t<int>(); return 0; }
} // namespace c_requires_b
namespace c_noexcept_r {
void t() noexcept(arity(key<int>()) == 1) {}
int go() { static_assert(noexcept(t())); return 0; }
} // namespace c_noexcept_r
namespace c_noexcept_b {
void t() noexcept(arity_b(box<int>()) == 1) {}
int go() { static_assert(noexcept(t())); return 0; }
} // namespace c_noexcept_b
namespace c_case_r {
int t(int x) { switch (x) { case arity(key<int>()): return 0; } return 1; }
int go() { return t(1); }
} // namespace c_case_r
namespace c_case_b {
int t(int x) { switch (x) { case arity_b(box<int>()): return 0; } return 1; }
int go() { return t(1); }
} // namespace c_case_b
namespace c_constexpr_local_r {
void t() { constexpr info k = key<int>(); use<k>(); }
int go() { t(); return 0; }
} // namespace c_constexpr_local_r
namespace c_constexpr_local_b {
void t() { constexpr Box k = box<int>(); useb<k>(); }
int go() { t(); return 0; }
} // namespace c_constexpr_local_b
namespace c_pack_r {
void t() { usepk<key<int>(), key<long>()>(); }
int go() { t(); return 0; }
} // namespace c_pack_r
namespace c_pack_b {
void t() { usepkb<box<int>(), box<long>()>(); }
int go() { t(); return 0; }
} // namespace c_pack_b
namespace c_member_fn_r {
struct M { void g() { use<key<int>()>(); } };
int go() { M{}.g(); return 0; }
} // namespace c_member_fn_r
namespace c_member_fn_b {
struct M { void g() { useb<box<int>()>(); } };
int go() { M{}.g(); return 0; }
} // namespace c_member_fn_b
namespace c_member_of_class_tmpl_r {
struct M { void g() { use<key<int>()>(); } };
int go() { M{}.g(); return 0; }
} // namespace c_member_of_class_tmpl_r
namespace c_member_of_class_tmpl_b {
struct M { void g() { useb<box<int>()>(); } };
int go() { M{}.g(); return 0; }
} // namespace c_member_of_class_tmpl_b
namespace c_lambda_r {
void t() { [] { use<key<int>()>(); }(); }
int go() { t(); return 0; }
} // namespace c_lambda_r
namespace c_lambda_b {
void t() { [] { useb<box<int>()>(); }(); }
int go() { t(); return 0; }
} // namespace c_lambda_b
namespace c_generic_lambda_r {
void t() { [](auto) { use<key<int>()>(); }(0); }
int go() { t(); return 0; }
} // namespace c_generic_lambda_r
namespace c_generic_lambda_b {
void t() { [](auto) { useb<box<int>()>(); }(0); }
int go() { t(); return 0; }
} // namespace c_generic_lambda_b
namespace c_nested_r {
void inner() { use<key<int>()>(); }
void outer() { inner(); }
int go() { outer(); return 0; }
} // namespace c_nested_r
namespace c_nested_b {
void inner() { useb<box<int>()>(); }
void outer() { inner(); }
int go() { outer(); return 0; }
} // namespace c_nested_b
namespace c_nested_class_r {
struct O { struct I { static void g() { use<key<int>()>(); } }; };
int go() { O::I::g(); return 0; }
} // namespace c_nested_class_r
namespace c_nested_class_b {
struct O { struct I { static void g() { useb<box<int>()>(); } }; };
int go() { O::I::g(); return 0; }
} // namespace c_nested_class_b

int main() {
  int bad = 0;
  bad += c_fn_arg_r::go();
  bad += c_fn_arg_b::go();
  bad += c_fn_arg_var_pack_r::go();
  bad += c_fn_arg_var_pack_b::go();
  bad += c_fn_arg_vartmpl_r::go();
  bad += c_fn_arg_vartmpl_b::go();
  bad += c_class_arg_r::go();
  bad += c_class_arg_b::go();
  bad += c_base_spec_r::go();
  bad += c_base_spec_b::go();
  bad += c_alias_r::go();
  bad += c_alias_b::go();
  bad += c_var_arg_r::go();
  bad += c_var_arg_b::go();
  bad += c_default_fn_r::go();
  bad += c_default_fn_b::go();
  bad += c_default_class_r::go();
  bad += c_default_class_b::go();
  bad += c_array_bound_r::go();
  bad += c_array_bound_b::go();
  bad += c_static_assert_r::go();
  bad += c_static_assert_b::go();
  bad += c_requires_r::go();
  bad += c_requires_b::go();
  bad += c_noexcept_r::go();
  bad += c_noexcept_b::go();
  bad += c_case_r::go();
  bad += c_case_b::go();
  bad += c_constexpr_local_r::go();
  bad += c_constexpr_local_b::go();
  bad += c_pack_r::go();
  bad += c_pack_b::go();
  bad += c_member_fn_r::go();
  bad += c_member_fn_b::go();
  bad += c_member_of_class_tmpl_r::go();
  bad += c_member_of_class_tmpl_b::go();
  bad += c_lambda_r::go();
  bad += c_lambda_b::go();
  bad += c_generic_lambda_r::go();
  bad += c_generic_lambda_b::go();
  bad += c_nested_r::go();
  bad += c_nested_b::go();
  bad += c_nested_class_r::go();
  bad += c_nested_class_b::go();
  puts(bad == 0 ? "ok" : "bad");
}
