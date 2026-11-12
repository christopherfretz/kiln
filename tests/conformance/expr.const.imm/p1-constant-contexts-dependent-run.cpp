// [expr.const.imm]/1.2 (P4101R1): as p1-constant-contexts-run.cpp, each
// case spelled dependently and instantiated: the constant expression is
// rebuilt by template instantiation, a manifestly constant-evaluated
// context exactly as when it was parsed, so the consteval-only results of
// the calls in it are allowed. Covers template arguments (function,
// class, base-specifier, alias, variable, default), array bounds,
// `static_assert`, requires-clauses, `noexcept`, `case`, a `constexpr`
// local used as a template argument, a pack expansion
// (`usepk<key<Rs>()...>`), member function templates, members of class
// templates, lambdas and generic lambdas, and calls two instantiation
// levels deep. Compared with GCC 16.1 (WP-1328).
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
template <class T> void t() { use<key<T>()>(); }
int go() { t<int>(); return 0; }
} // namespace c_fn_arg_r
namespace c_fn_arg_b {
template <class T> void t() { useb<box<T>()>(); }
int go() { t<int>(); return 0; }
} // namespace c_fn_arg_b
namespace c_fn_arg_var_pack_r {
template <class... Rs> void t() { use<key_of<Rs...>()>(); }
int go() { t<int, long>(); return 0; }
} // namespace c_fn_arg_var_pack_r
namespace c_fn_arg_var_pack_b {
template <class... Rs> void t() { useb<box_of<Rs...>()>(); }
int go() { t<int, long>(); return 0; }
} // namespace c_fn_arg_var_pack_b
namespace c_fn_arg_vartmpl_r {
template <class... Rs> void t() { use<vk<Rs...>>(); }
int go() { t<int>(); return 0; }
} // namespace c_fn_arg_vartmpl_r
namespace c_fn_arg_vartmpl_b {
template <class... Rs> void t() { useb<vkb<Rs...>>(); }
int go() { t<int>(); return 0; }
} // namespace c_fn_arg_vartmpl_b
namespace c_class_arg_r {
template <class T> int t() { return C<key<T>()>::v; }
int go() { return t<int>() - 1; }
} // namespace c_class_arg_r
namespace c_class_arg_b {
template <class T> int t() { return CB<box<T>()>::v; }
int go() { return t<int>() - 1; }
} // namespace c_class_arg_b
namespace c_base_spec_r {
template <class T> struct D : C<key<T>()> {};
int go() { return D<int>::v - 1; }
} // namespace c_base_spec_r
namespace c_base_spec_b {
template <class T> struct D : CB<box<T>()> {};
int go() { return D<int>::v - 1; }
} // namespace c_base_spec_b
namespace c_alias_r {
template <class T> using A = C<key<T>()>;
int go() { return A<int>::v - 1; }
} // namespace c_alias_r
namespace c_alias_b {
template <class T> using A = CB<box<T>()>;
int go() { return A<int>::v - 1; }
} // namespace c_alias_b
namespace c_var_arg_r {
template <class T> int t() { return var<key<T>()>; }
int go() { return t<int>() - 1; }
} // namespace c_var_arg_r
namespace c_var_arg_b {
template <class T> int t() { return varb<box<T>()>; }
int go() { return t<int>() - 1; }
} // namespace c_var_arg_b
namespace c_default_fn_r {
template <class T, info H = key<T>()> void d() {}
int go() { d<int>(); return 0; }
} // namespace c_default_fn_r
namespace c_default_fn_b {
template <class T, Box H = box<T>()> void d() {}
int go() { d<int>(); return 0; }
} // namespace c_default_fn_b
namespace c_default_class_r {
template <class T, info H = key<T>()> struct DC {};
int go() { DC<int> x; (void)x; return 0; }
} // namespace c_default_class_r
namespace c_default_class_b {
template <class T, Box H = box<T>()> struct DCB {};
int go() { DCB<int> x; (void)x; return 0; }
} // namespace c_default_class_b
namespace c_array_bound_r {
template <class T> int t() { int a[arity(key<T>())] = {}; return sizeof a; }
int go() { return t<int>() == 4 ? 0 : 1; }
} // namespace c_array_bound_r
namespace c_array_bound_b {
template <class T> int t() { int a[arity_b(box<T>())] = {}; return sizeof a; }
int go() { return t<int>() == 4 ? 0 : 1; }
} // namespace c_array_bound_b
namespace c_static_assert_r {
template <class T> void t() { static_assert(arity(key<T>()) == 1); }
int go() { t<int>(); return 0; }
} // namespace c_static_assert_r
namespace c_static_assert_b {
template <class T> void t() { static_assert(arity_b(box<T>()) == 1); }
int go() { t<int>(); return 0; }
} // namespace c_static_assert_b
namespace c_requires_r {
template <class T> requires (arity(key<T>()) == 1) void t() {}
int go() { t<int>(); return 0; }
} // namespace c_requires_r
namespace c_requires_b {
template <class T> requires (arity_b(box<T>()) == 1) void t() {}
int go() { t<int>(); return 0; }
} // namespace c_requires_b
namespace c_noexcept_r {
template <class T> void t() noexcept(arity(key<T>()) == 1) {}
int go() { static_assert(noexcept(t<int>())); return 0; }
} // namespace c_noexcept_r
namespace c_noexcept_b {
template <class T> void t() noexcept(arity_b(box<T>()) == 1) {}
int go() { static_assert(noexcept(t<int>())); return 0; }
} // namespace c_noexcept_b
namespace c_case_r {
template <class T> int t(int x) { switch (x) { case arity(key<T>()): return 0; } return 1; }
int go() { return t<int>(1); }
} // namespace c_case_r
namespace c_case_b {
template <class T> int t(int x) { switch (x) { case arity_b(box<T>()): return 0; } return 1; }
int go() { return t<int>(1); }
} // namespace c_case_b
namespace c_constexpr_local_r {
template <class T> void t() { constexpr info k = key<T>(); use<k>(); }
int go() { t<int>(); return 0; }
} // namespace c_constexpr_local_r
namespace c_constexpr_local_b {
template <class T> void t() { constexpr Box k = box<T>(); useb<k>(); }
int go() { t<int>(); return 0; }
} // namespace c_constexpr_local_b
namespace c_pack_r {
template <class... Rs> void t() { usepk<key<Rs>()...>(); }
int go() { t<int, long>(); return 0; }
} // namespace c_pack_r
namespace c_pack_b {
template <class... Rs> void t() { usepkb<box<Rs>()...>(); }
int go() { t<int, long>(); return 0; }
} // namespace c_pack_b
namespace c_member_fn_r {
struct M { template <class T> void g() { use<key<T>()>(); } };
int go() { M{}.g<int>(); return 0; }
} // namespace c_member_fn_r
namespace c_member_fn_b {
struct M { template <class T> void g() { useb<box<T>()>(); } };
int go() { M{}.g<int>(); return 0; }
} // namespace c_member_fn_b
namespace c_member_of_class_tmpl_r {
template <class T> struct M { void g() { use<key<T>()>(); } };
int go() { M<int>{}.g(); return 0; }
} // namespace c_member_of_class_tmpl_r
namespace c_member_of_class_tmpl_b {
template <class T> struct M { void g() { useb<box<T>()>(); } };
int go() { M<int>{}.g(); return 0; }
} // namespace c_member_of_class_tmpl_b
namespace c_lambda_r {
template <class T> void t() { [] { use<key<T>()>(); }(); }
int go() { t<int>(); return 0; }
} // namespace c_lambda_r
namespace c_lambda_b {
template <class T> void t() { [] { useb<box<T>()>(); }(); }
int go() { t<int>(); return 0; }
} // namespace c_lambda_b
namespace c_generic_lambda_r {
void t() { [](auto x) { use<key<decltype(x)>()>(); }(0); }
int go() { t(); return 0; }
} // namespace c_generic_lambda_r
namespace c_generic_lambda_b {
void t() { [](auto x) { useb<box<decltype(x)>()>(); }(0); }
int go() { t(); return 0; }
} // namespace c_generic_lambda_b
namespace c_nested_r {
template <class T> void inner() { use<key<T>()>(); }
template <class T> void outer() { inner<T>(); }
int go() { outer<int>(); return 0; }
} // namespace c_nested_r
namespace c_nested_b {
template <class T> void inner() { useb<box<T>()>(); }
template <class T> void outer() { inner<T>(); }
int go() { outer<int>(); return 0; }
} // namespace c_nested_b
namespace c_nested_class_r {
template <class T> struct O { template <class U> struct I { static void g() { use<key<U>()>(); } }; };
int go() { O<long>::I<int>::g(); return 0; }
} // namespace c_nested_class_r
namespace c_nested_class_b {
template <class T> struct O { template <class U> struct I { static void g() { useb<box<U>()>(); } }; };
int go() { O<long>::I<int>::g(); return 0; }
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
