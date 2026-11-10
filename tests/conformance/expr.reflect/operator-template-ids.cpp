// [expr.reflect]/7 (P2996R13), [temp.names]/1, [over.over]/3,
// [temp.arg.explicit]/4: an operator-function-id or literal-operator-id
// template-id (`operator== <int>`, `operator""_x<'1'>`) is a template-id,
// so `^^` of one reflects the unique function template specialization its
// explicit arguments select, qualified or not, at namespace or class
// scope, as an identifier template-id does (`^^ns::plain<int>`). Before
// WP-1246 the parser stopped after `^^ns::operator==` and rejected the
// `<int>` ("expected '(' for function-style cast", a reference-workload
// finding). In a template the arguments may be dependent
// (`^^ns::operator==<T>`, `^^ns::plain<T>`, `^^T::template operator()<int>`):
// the reflection resolves at instantiation (spec/reflection-repr.md §2).
// display_string_of agrees with GCC 16.1 character for character.
// Compared with GCC 16.1 (spec/harness.md "GCC oracle").
//@ expect: compile-ok
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
#include <meta>
#include <string_view>
using std::meta::info;
namespace ns {
struct A { int v; };
template <class T> bool operator==(const A& a, T b) { return a.v == b; }
template <class T> int operator+(const A& a, T b) { return a.v + b; }
template <char... C> constexpr int operator""_x() { return sizeof...(C); }
template <class T> T plain(T t) { return t; }
struct S {
  template <class T> int operator()(T) const { return 1; }
  template <class T> bool operator<(T) const { return true; }
};
} // namespace ns
using namespace ns;
using std::string_view;

// The reference workload's reduced form.
constexpr info eq = ^^ns::operator== <int>;
static_assert(std::meta::is_function(eq));
static_assert(!std::meta::is_function_template(eq));
static_assert(std::meta::is_operator_function(eq));
static_assert(std::meta::has_template_arguments(eq));
static_assert(std::meta::template_of(eq) == ^^ns::operator==);
static_assert(std::meta::template_arguments_of(eq)[0] == ^^int);
static_assert(std::meta::type_of(eq) == ^^bool(const ns::A&, int));
static_assert(std::meta::parent_of(eq) == ^^ns);
static_assert(std::meta::display_string_of(eq) ==
              string_view("bool ns::operator==(const A&, T) [with T = int]"));

// Unqualified (found through the using-directive), and with no space
// before `<`.
static_assert(^^operator==<int> == eq);
static_assert(^^::ns::operator==<int> == eq);
static_assert(^^ns::operator==<long> != eq);
static_assert(std::meta::display_string_of(^^ns::operator+<long>) ==
              string_view("int ns::operator+(const A&, T) [with T = long int]"));

// A literal-operator-id template-id.
constexpr info x12 = ^^ns::operator""_x<'1', '2'>;
static_assert(std::meta::is_function(x12));
static_assert(std::meta::is_literal_operator(x12));
static_assert(std::meta::template_of(x12) == ^^ns::operator""_x);
static_assert(^^operator""_x<'1'> != x12);
static_assert(std::meta::display_string_of(x12) ==
              string_view("constexpr int ns::operator\"\"_x() [with char ...C = {'1', '2'}]"));

// Member operator function templates: no object is needed.
constexpr info call = ^^ns::S::operator()<int>;
static_assert(std::meta::is_function(call));
static_assert(std::meta::parent_of(call) == ^^ns::S);
static_assert(std::meta::template_of(call) == ^^S::operator());
static_assert(std::meta::display_string_of(call) ==
              string_view("int ns::S::operator()(T) const [with T = int]"));
static_assert(std::meta::display_string_of(^^S::operator< <char>) ==
              string_view("bool ns::S::operator<(T) const [with T = char]"));

// The identifier forms, for comparison.
static_assert(^^ns::plain<int> == ^^plain<int>);
static_assert(std::meta::display_string_of(^^ns::plain<int>) ==
              string_view("T ns::plain(T) [with T = int]"));

// Dependent template arguments resolve at instantiation.
template <class T> consteval info eq_of() { return ^^ns::operator==<T>; }
template <class T> consteval info eq_unq() { return ^^operator==<T>; }
template <class T> consteval info plain_of() { return ^^ns::plain<T>; }
template <char C> consteval info lit_of() { return ^^ns::operator""_x<C>; }
template <class T> consteval info call_of() { return ^^T::template operator()<int>; }
template <class... Ts> consteval int count() {
  info rs[] = {^^ns::plain<Ts>...};
  return sizeof(rs) / sizeof(info);
}
static_assert(eq_of<int>() == eq);
static_assert(eq_unq<int>() == eq);
static_assert(eq_of<char>() == ^^ns::operator==<char>);
static_assert(plain_of<int>() == ^^ns::plain<int>);
static_assert(lit_of<'1'>() == ^^ns::operator""_x<'1'>);
static_assert(count<int, char, long>() == 3);

// The same names outside `^^`: the address and a call.
bool (*p1)(const A&, int) = &ns::operator==<int>;
bool (*p2)(const A&, int) = &operator==<int>;
int (*p3)() = &ns::operator""_x<'a'>;
constexpr int lit = ns::operator""_x<'a', 'b'>();
static_assert(lit == 2);
bool c1 = ns::operator==<int>(A{1}, 1);
bool c2 = operator==<int>(A{1}, 1);
