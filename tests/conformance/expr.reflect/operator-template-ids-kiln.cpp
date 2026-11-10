// [expr.reflect]/7 (P2996R13), [temp.mem]/6, [temp.names]/5: two operand
// forms where GCC 16.1 diverges from the standard, so kiln is the
// reference here (no oracle; WP-1246).
// - `^^S::operator int*` with only a conversion function template
//   `template <class T> operator T*()`: the conversion-function-id names
//   the specialization deduced for `int*` ([temp.mem]/6: it is referenced
//   as a non-template conversion function to the same type is), so the
//   reflection is that function, as `&S::operator int*` names it
//   (temp.mem/conversion-template-qualified-id-run.cpp). GCC 16.1 reflects
//   the template itself.
// - `^^T::template operator()<int>` instantiated with a class that has a
//   member `operator()` template: the specialization, as
//   `^^S::operator()<int>` reflects it. GCC 16.1 rejects the
//   instantiation ("'operator()' not defined").
//@ expect: compile-ok
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ skip(gcc): GCC 16.1 diverges on both forms (see above)
//@ strict-diags
#include <meta>
#include <string_view>
using std::meta::info;
using std::string_view;
struct S {
  template <class T> operator T*() const { return nullptr; }
  template <class T> int operator()(T) const { return 1; }
};

constexpr info conv = ^^S::operator int*;
static_assert(std::meta::is_function(conv));
static_assert(std::meta::is_conversion_function(conv));
static_assert(!std::meta::is_conversion_function_template(conv));
static_assert(std::meta::is_conversion_function_template(std::meta::template_of(conv)));
static_assert(std::meta::parent_of(conv) == ^^S);
static_assert(^^S::operator long* != conv);
static_assert(std::meta::display_string_of(conv) ==
              string_view("S::operator T*() const [with T = int]"));

template <class T> consteval info call_of() { return ^^T::template operator()<int>; }
template <class T> consteval info conv_of() { return ^^T::operator int*; }
static_assert(call_of<S>() == ^^S::operator()<int>);
static_assert(conv_of<S>() == conv);
