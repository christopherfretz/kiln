// [over.match.oper]/3.1: member candidates are looked up only in a
// complete class type; an operand of an incomplete class (a specialization
// of a template that is only declared) contributes none and is not an
// error, so a non-member operator is found. The libc++
// `stream_insert_decl_present` pattern (`ostream&` from <iosfwd>).
//@ expect: compile-ok
//@ strict-diags
namespace n {
template <class C> struct traits {};
template <class C, class T = traits<C>> struct ostr;
using ostream = ostr<char>;
template <class C, class T = traits<C>> struct sv {};
template <class C, class T>
ostr<C, T>& operator<<(ostr<C, T>& os, sv<C, T> s);
} // namespace n

template <class T> T&& declval();
template <class SV, class = void> struct HasDecl {
  static const bool value = false;
};
template <class SV>
struct HasDecl<SV, decltype(static_cast<void>(declval<n::ostream&>() << declval<SV&>()))> {
  static const bool value = true;
};
static_assert(HasDecl<n::sv<char>>::value);

void f(n::ostream& os, n::sv<char> s) { os << s; }
