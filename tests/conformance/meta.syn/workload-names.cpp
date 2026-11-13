// [meta.syn] (P2996R13 and the papers GCC 16.1 adopts with it): libstdc++
// 16.1's real <meta>, through --gcc-toolchain, with only <vector>,
// <string_view>, <array> and <ranges> beside it. Every name reflection-heavy
// code uses most (dealias, display_string_of, is_type, type_of, ...,
// define_static_string, define_static_array) evaluates as GCC 16.1 does:
// string_view, vector<info>, access_context, std::strong_ordering
// (type_order), std::meta::exception, source_location and the range
// templates are the header's own classes. The WP-484 exit criterion; GCC
// compiles and runs the same file, and the exit status must agree.
//@ expect: run
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
//@ exit: 6
#include <meta>
#include <vector>
#include <string_view>
#include <array>
#include <ranges>

namespace N {
struct B { virtual ~B(); };
struct [[=1, =2.5]] S : B {
  int a;
  static int b;
  const int c = 0;
  int& r;
  void f(int x, double);
  S(const S&) = delete;
  S(int);
  ~S();
  operator int();
  bool operator==(const S&) const;
  virtual void g();
  template <class T> void tf();
  int arr[3];
};
enum E { x, y, z };
enum class EC : short { k };
template <class T> struct T1 { T v; };
template <class T> void ft(T);
extern "C" void cfn();
void plain(int);
int v;
}  // namespace N
using A = N::S;
using IP = int*;
constexpr auto ctx = std::meta::access_context::unchecked();
using namespace std::meta;

// dealias, display_string_of, is_type, type_of, is_function
static_assert(dealias(^^A) == ^^N::S);
static_assert(dealias(^^IP) == ^^int*);
static_assert(dealias(^^int) == ^^int);
static_assert(display_string_of(^^int) == "int");
static_assert(!display_string_of(^^N::S::f).empty());
static_assert(is_type(^^int) && is_type(^^A) && !is_type(^^N::v));
static_assert(type_of(^^N::S::a) == ^^int);
static_assert(type_of(^^N::v) == ^^int);
static_assert(is_function(^^N::S::f) && is_function(^^N::plain) && !is_function(^^N::v));
// is_class_type, parent_of, parameters_of, identifier_of, has_identifier
static_assert(is_class_type(^^N::S) && !is_class_type(^^N::E));
static_assert(parent_of(^^N::S) == ^^N && parent_of(^^N::S::a) == ^^N::S);
static_assert(parameters_of(^^N::S::f).size() == 2);
static_assert(type_of(parameters_of(^^N::S::f)[1]) == ^^double);
static_assert(identifier_of(parameters_of(^^N::S::f)[0]) == "x");
static_assert(identifier_of(^^N::S) == "S" && identifier_of(^^N::S::a) == "a");
static_assert(has_identifier(^^N::S) && !has_identifier(^^int));
// has_template_arguments, template_arguments_of, extract, return_type_of
static_assert(has_template_arguments(^^N::T1<int>) && !has_template_arguments(^^N::S));
static_assert(template_arguments_of(^^N::T1<long>).size() == 1);
static_assert(template_arguments_of(^^N::T1<long>)[0] == ^^long);
static_assert(extract<int>(reflect_constant(7)) == 7);
static_assert(return_type_of(^^N::S::operator==) == ^^bool);
static_assert(return_type_of(^^N::S::f) == ^^void);
// members_of, access_context::unchecked, template_of, is_pointer_type
static_assert(members_of(^^N::S, ctx).size() > 8);
static_assert(template_of(^^N::T1<int>) == ^^N::T1);
static_assert(is_pointer_type(^^int*) && !is_pointer_type(^^int&));
// is_namespace, is_template, is_complete_type, is_public, is_constructor
static_assert(is_namespace(^^N) && is_namespace(^^::) && !is_namespace(^^N::S));
static_assert(is_template(^^N::T1) && is_template(^^N::ft) && !is_template(^^N::S));
static_assert(is_complete_type(^^N::S) && !is_complete_type(^^void));
static_assert(is_public(^^N::S::a));
consteval int ctors() {
  int n = 0;
  for (info m : members_of(^^N::S, ctx)) n += is_constructor(m);
  return n;
}
static_assert(ctors() == 2 && !is_constructor(^^N::S::f));
// annotations_of, is_static_member, is_enum_type, is_operator_function, is_class_member
static_assert(annotations_of(^^N::S).size() == 2);
static_assert(extract<int>(annotations_of(^^N::S)[0]) == 1);
static_assert(annotations_of_with_type(^^N::S, ^^double).size() == 1);
static_assert(extract<double>(constant_of(annotations_of(^^N::S)[1])) == 2.5);
static_assert(is_static_member(^^N::S::b) && !is_static_member(^^N::S::a));
static_assert(is_enum_type(^^N::E) && is_enum_type(^^N::EC) && !is_enum_type(^^int));
static_assert(is_operator_function(^^N::S::operator==) && !is_operator_function(^^N::S::f));
static_assert(is_class_member(^^N::S::a) && !is_class_member(^^N::v));
// is_same_type, operator_of, is_virtual, is_reference_type, is_destructor
static_assert(is_same_type(^^int, ^^int) && !is_same_type(^^int, ^^long));
static_assert(is_same_type(^^A, ^^N::S));
static_assert(operator_of(^^N::S::operator==) == op_equals_equals);
static_assert(symbol_of(op_equals_equals) == "==");
static_assert(u8symbol_of(op_plus) == u8"+");
static_assert(is_virtual(^^N::S::g) && !is_virtual(^^N::S::f));
static_assert(is_reference_type(^^int&) && is_reference_type(^^int&&) && !is_reference_type(^^int));
static_assert(is_destructor(^^N::S::~S));
// is_const, is_const_type, bases_of, is_deleted, substitute, reflect_constant
static_assert(is_const(^^N::S::c) && !is_const(^^N::S::a));
static_assert(is_const_type(^^const int) && !is_const_type(^^int));
static_assert(bases_of(^^N::S, ctx).size() == 1);
static_assert(type_of(bases_of(^^N::S, ctx)[0]) == ^^N::B);
static_assert(substitute(^^N::T1, {^^int}) == ^^N::T1<int>);
static_assert(can_substitute(^^N::T1, {^^int}));
constexpr std::array<info, 1> targs{^^char};
static_assert(substitute(^^N::T1, targs) == ^^N::T1<char>);
static_assert(substitute(^^N::T1, std::vector<info>{^^short}) == ^^N::T1<short>);
static_assert(reflect_constant(3) == reflect_constant(3));
static_assert(reflect_constant(3) != reflect_constant(4));
// is_function_type, constant_of, is_copy_constructible_type, is_variable
static_assert(is_function_type(^^void(int)) && !is_function_type(^^int));
static_assert(constant_of(^^N::E::y) == reflect_constant(N::y));
static_assert(is_copy_constructible_type(^^int) && !is_copy_constructible_type(^^N::S));
static_assert(is_variable(^^N::v) && !is_variable(^^N::S::a));
// is_lvalue_reference_type, is_array_type, is_function_template, is_integral_type
static_assert(is_lvalue_reference_type(^^int&) && !is_lvalue_reference_type(^^int&&));
static_assert(is_array_type(^^int[3]) && is_array_type(type_of(^^N::S::arr)));
static_assert(is_function_template(^^N::ft) && !is_function_template(^^N::T1));
static_assert(is_integral_type(^^int) && is_integral_type(^^bool) && !is_integral_type(^^float));
// is_conversion_function, is_class_template, is_nonstatic_data_member
static_assert(is_conversion_function(^^N::S::operator int) && !is_conversion_function(^^N::S::f));
static_assert(is_class_template(^^N::T1) && !is_class_template(^^N::ft));
static_assert(is_nonstatic_data_member(^^N::S::a) && !is_nonstatic_data_member(^^N::S::b));
static_assert(enumerators_of(^^N::E).size() == 3);
static_assert(identifier_of(enumerators_of(^^N::E)[2]) == "z");
static_assert(nonstatic_data_members_of(^^N::S, ctx).size() == 4);
// type_order, range traits, define_static_string/array, reflect_constant_*
static_assert(type_order(^^int, ^^int) == 0);
static_assert(type_order(^^int, ^^long) != 0);
static_assert((type_order(^^int, ^^long) < 0) == (type_order(^^long, ^^int) > 0));
static_assert(is_constructible_type(^^N::S, {^^int}));
static_assert(!is_constructible_type(^^N::S, {}));
static_assert(is_invocable_type(^^void(*)(int), {^^int}));
static_assert(common_type({^^int, ^^long}) == ^^long);
constexpr const char* hs = std::define_static_string(std::string_view("hi"));
static_assert(hs[0] == 'h' && hs[2] == '\0');
constexpr std::array<int, 3> arr{1, 2, 3};
constexpr auto sp = std::define_static_array(arr);
static_assert(sp.size() == 3 && sp[2] == 3);
static_assert(extract<const char*>(reflect_constant_string("ab"))[1] == 'b');
static_assert(is_array_type(type_of(reflect_constant_array(std::vector<int>{4, 5}))));
// std::meta::exception
consteval bool thrown() {
  try { (void)type_of(^^N); } catch (const std::meta::exception& e) {
    return e.from() == ^^type_of && !std::string_view(e.what()).empty();
  }
  return false;
}
static_assert(thrown());
// source_location_of, is_string_literal
static_assert(source_location_of(^^N::S).line() == 24);
static_assert(std::is_string_literal("x"));
constexpr int answer = []{ int n = 0; for (info m : nonstatic_data_members_of(^^N::S, ctx)) n += identifier_of(m).size(); return n; }();
int main() { return answer; }
