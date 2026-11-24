// [except.throw]/3: the exception object types of
// constexpr-uncaught-type-spelling.cpp as clang 19 and clang 22 print
// them in their own diagnostics, as canonical types (a template's
// argument): an inline namespace is left out only where it is redundant,
// at every depth (`app::lib::error`; `m::v1::e` stays, `m::e` would be
// ambiguous), the unnamed namespace prints `(anonymous namespace)::`, and
// a specialization's arguments follow the same rule. kiln's
// uncaught-exception note spells them alike (WP-3408).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 28:43 error "no type named 'missing' in 'lib::error'"
//@ diag: 29:19 note "in instantiation of template class 'Z<lib::error>' requested here"
//@ diag: 28:43 error "no type named 'missing' in 'app::lib::error'"
//@ diag: 30:24 note "in instantiation of template class 'Z<app::lib::error>' requested here"
//@ diag: 28:43 error "no type named 'missing' in '(anonymous namespace)::error'"
//@ diag: 31:10 note "in instantiation of template class 'Z<(anonymous namespace)::error>' requested here"
//@ diag: 28:43 error "no type named 'missing' in 'outer::box<lib::error>'"
//@ diag: 32:31 note "in instantiation of template class 'Z<outer::box<lib::error>>' requested here"
//@ diag: 28:43 error "no type named 'missing' in 'outer::box<int>::nested'"
//@ diag: 33:28 note "in instantiation of template class 'Z<outer::box<int>::nested>' requested here"
//@ diag: 28:43 error "no type named 'missing' in 'm::v1::e'"
//@ diag: 34:13 note "in instantiation of template class 'Z<m::v1::e>' requested here"
namespace lib { inline namespace v0 { struct error { int c; }; } }
namespace app { namespace lib { inline namespace v0 { struct error { int c; }; } } }
namespace { struct error { int c; }; }
namespace outer { template <class T> struct box { int c; struct nested { int c; }; }; }
namespace m { inline namespace v1 { struct e { int c; }; } inline namespace v2 { struct e { int c; }; } }
template <class T> struct Z { typename T::missing m; };
Z<lib::v0::error> z1;
Z<app::lib::v0::error> z2;
Z<error> z3;
Z<outer::box<lib::v0::error>> z4;
Z<outer::box<int>::nested> z5;
Z<m::v1::e> z6;
