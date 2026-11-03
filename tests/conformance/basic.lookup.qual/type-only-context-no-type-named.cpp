// [basic.lookup.qual]/1, [dcl.type.simple]/2: in a type-only context (an
// alias-declaration, a typedef, a type-parameter default) a qualified name
// whose last component names no type is a lookup error ("no type named 'x'
// in ..."), not a syntax error; an explicit specialization named as the
// context prints its arguments as written ([temp.expl.spec]/1), so
// 'std::hash<monostate>' where a use would print 'std::hash<std::monostate>'.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 32:48 error "no type named 'argument_type' in 'std::hash<V<int>>'"
//@ diag: 33:56 error "no type named 'argument_type' in 'std::hash<monostate>'"
//@ diag: 34:28 error "no type named 'argument_type' in 'std::hash<int>'"
//@ diag: 36:15 error "no type named 'nope' in 'P'"
//@ diag: 37:12 error "no type named 'nope' in 'P'"
//@ diag: 40:15 error "no type named 'f' in namespace 'N'"
//@ diag: 41:15 error "no type named 'nope' in namespace 'N'"
//@ diag: 42:15 error "no type named 's' in 'P'"
//@ diag: 43:14 error "no type named 'nope' in the global namespace"
//@ diag: 44:20 error "no type named 'nope' in 'Q<int>'"
//@ diag: 45:21 error "no type named 'nope' in 'P'"
//@ diag: 46:24 error "no type named 'nope' in 'P'"
//@ diag: 47:36 error "no member named 'q' in 'std::hash<monostate>'"
namespace std {
template <class T> struct hash;
template <> struct hash<int> { int operator()(int) const; };
struct monostate {};
template <> struct hash<monostate> {};
} // namespace std
template <class T> struct V {};
namespace std {
template <class T> struct hash<V<T>> {};
} // namespace std
using A1 [[maybe_unused]] = std::hash<V<int>>::argument_type;
using A2 [[maybe_unused]] = std::hash<std::monostate>::argument_type;
using A3 = std::hash<int>::argument_type;
struct P { static int s; };
using A4 = P::nope;
typedef P::nope A5;
namespace N { void f(); int v; }
template <class T> struct Q {};
using B1 = N::f;
using B2 = N::nope;
using B3 = P::s;
using B4 = ::nope;
using B5 = Q<int>::nope*;
using B6 = const P::nope&;
template <class T = P::nope> struct R;
int z = std::hash<std::monostate>::q;
