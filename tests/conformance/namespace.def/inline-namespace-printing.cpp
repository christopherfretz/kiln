// [namespace.def]: an inline namespace is left out of a printed type or
// declaration name only when the enclosing namespace's lookup finds as many
// declarations of the name (clang isRedundantInlineQualifierFor).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 26:7 error "no viable conversion from 'std::vec' to 'int'"
//@ diag: 27:7 error "no viable conversion from 'c::d::X' to 'int'"
//@ diag: 28:7 error "no viable conversion from 'c::e::X' to 'int'"
//@ diag: 29:7 error "no viable conversion from 'p::Y' to 'int'"
//@ diag: 30:7 error "no viable conversion from 's::t::Z' to 'int'"
//@ diag: 31:7 error "no viable conversion from 'u::w::W' to 'int'"
//@ diag: 32:6 error "attempt to use a deleted function"
//@ diag: 21:54 note "'g' has been explicitly marked deleted here"
//@ diag: 20:98 error "static assertion failed due to requirement 'sizeof(int) == 0'"
//@ diag: 33:8 note "in instantiation of function template specialization 'std::f<int>' requested here"
//@ diag: 20:108 note "expression evaluates to '4 == 0'"
//@ diag: 35:79 error "static assertion failed due to requirement 'sizeof(char) == 0'"
//@ diag: 36:19 note "in instantiation of function template specialization 'c::d::k<char>' requested here"
//@ diag: 35:89 note "expression evaluates to '1 == 0'"
namespace std { inline namespace __1 { struct vec {}; template<class T> void f() { static_assert(sizeof(T) == 0); } } }
namespace c { inline namespace d { struct X {}; void g(int) = delete; } inline namespace e { struct X {}; } }
namespace p { inline namespace q { inline namespace r { struct Y {}; } } }
namespace s { inline namespace t { struct Z {}; } int Z; }
namespace u { inline namespace v { namespace w { struct W {}; } } }
void h() {
  int i = std::vec{};
  int j = c::d::X{};
  int k = c::e::X{};
  int l = p::Y{};
  int m = s::t::Z{};
  int n = u::w::W{};
  c::g(1);
  std::f<int>();
}
namespace c { inline namespace d { template<class T> void k() { static_assert(sizeof(T) == 0); } } inline namespace e { void k(int); } }
void h2() { c::d::k<char>(); }
