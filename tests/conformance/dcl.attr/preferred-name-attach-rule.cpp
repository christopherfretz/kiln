// clang extension preferred_name ([dcl.attr.grammar]/5 vendor attribute):
// a specialization prints its preferred name only once it has the
// attribute, which it takes when it is instantiated (from the primary
// template) or when a template-id names it after the attribute is
// declared. A typedef written before the attribute names it too early,
// so an uninstantiated pointee prints as the template-id. A substituted
// template argument is canonical: `W<T>` with `T = s::pl` has no `aka`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 29:45 error "static assertion failed due to requirement '__is_same(s::basic_string<char> *, int)'"
//@ diag: 30:14 note "in instantiation of template class 'Z<s::basic_string<char>>' requested here"
//@ diag: 31:45 error "static assertion failed due to requirement 's::is_int<W<s::plain>>::value'"
//@ diag: 32:10 note "in instantiation of template class 'Y<s::plain>' requested here"
//@ diag: 29:45 error "static assertion failed due to requirement '__is_same(s::u16 *, int)'"
//@ diag: 34:11 note "in instantiation of template class 'Z<s::u16>' requested here"
//@ diag: 35:45 error "static assertion failed due to requirement 's::is_int<W<s::plain>>::value'"
//@ diag: 36:12 note "in instantiation of function template specialization 'f<s::plain>' requested here"
namespace s {
template <class C> struct basic_string;
using string = basic_string<char>;
using u16 = basic_string<char16_t>;
template <class C>
struct __attribute__((__preferred_name__(string), __preferred_name__(u16))) basic_string { int x; };
struct plain {};
using pl = plain;
template <class T> struct is_int { static constexpr bool value = false; };
}
template <class T> struct W {};
template <class T> struct Z { static_assert(__is_same(T *, int)); };
Z<s::string> z1;
template <class T> struct Y { static_assert(s::is_int<W<T>>::value); };
Y<s::pl> y1;
s::u16 named;
Z<s::u16> z2;
template <class T> void f() { static_assert(s::is_int<W<T>>::value); }
void g() { f<s::pl>(); }
