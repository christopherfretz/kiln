// clang extension preferred_name ([dcl.attr.grammar]/5 vendor attribute):
// on a class template, names a typedef of one of its specializations, which
// diagnostics then print instead of the template-id (libc++ std::string);
// the argument must be such a typedef, the subject a class template.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 28:45 error "static assertion failed due to requirement '__is_same(s::wstring *, int)'"
//@ diag: 29:29 note "in instantiation of template class 'X<s::wstring>' requested here"
//@ diag: 28:45 error "static assertion failed due to requirement '__is_same(s::basic_string<char16_t> *, int)'"
//@ diag: 30:30 note "in instantiation of template class 'X<s::basic_string<char16_t>>' requested here"
//@ diag: 32:7 error "no viable conversion from 's::string' (aka 'basic_string<char>') to 'int'"
//@ diag: 33:17 error "static assertion failed due to requirement 's::is_int<s::string>::value'"
//@ diag: 39:42 error "argument 'ds' (aka 'D<char>') to 'preferred_name' attribute is not a typedef for a specialization of 'B'"
//@ diag: 38:7 note "'ds' declared here"
//@ diag: 40:23 warning "'preferred_name' attribute only applies to class templates"
//@ diag: 42:42 error "argument 'B<int>' to 'preferred_name' attribute is not a typedef for a specialization of 'B'"
//@ diag: 43:42 error "'preferred_name' attribute takes one argument"
namespace s {
template <class C> struct basic_string;
using string = basic_string<char>;
using wstring = basic_string<wchar_t>;
template <class C>
struct __attribute__((__preferred_name__(string), __preferred_name__(wstring))) basic_string {
  int x;
};
template <class T> struct is_int { static constexpr bool value = false; };
}
template <class T> struct X { static_assert(__is_same(T *, int)); };
X<s::basic_string<wchar_t>> x;
X<s::basic_string<char16_t>> y;
void f(s::string *p) {
  int a = *p;
  static_assert(s::is_int<s::basic_string<char>>::value);
}
template <class C> struct B;
template <class C> struct D;
using bs = B<char>;
using ds = D<char>;
template <class C> struct __attribute__((preferred_name(ds))) B;
struct __attribute__((preferred_name(int))) S {};
template <class C> struct [[clang::preferred_name(bs)]] B;
template <class C> struct __attribute__((preferred_name(B<int>))) B;
template <class C> struct __attribute__((preferred_name)) B;
