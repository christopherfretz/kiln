// [meta.unary.prop] is_trivially_copyable, [basic.types.general]/9,
// [class.prop]/1: clang 22 explains a failed __is_trivially_copyable,
// std::is_trivially_copyable_v or std::is_trivially_copyable::value
// requirement: virtual and non-trivially-copyable bases, members, a
// user-provided destructor and copy/move operations, a reference type, and
// where the class is defined (an array or void gets the first note only).
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=22
//@ diag: 82:15 error "static assertion failed due to requirement 'std::is_trivially_copyable_v<NT>'"
//@ diag: 82:15 note "'NT' is not trivially copyable"
//@ diag: 82:15 note "because it has a user provided copy constructor"
//@ diag: 72:8 note "'NT' defined here"
//@ diag: 83:15 error "static assertion failed due to requirement 'std::is_trivially_copyable<NT>::value'"
//@ diag: 83:15 note "'NT' is not trivially copyable"
//@ diag: 83:15 note "because it has a user provided copy constructor"
//@ diag: 72:8 note "'NT' defined here"
//@ diag: 84:15 error "static assertion failed due to requirement '__is_trivially_copyable(B)'"
//@ diag: 84:15 note "'B' is not trivially copyable"
//@ diag: 84:15 note "because it has a virtual base 'V'"
//@ diag: 84:15 note "because it has a non-trivially-copyable base 'V'"
//@ diag: 74:8 note "'B' defined here"
//@ diag: 85:15 error "static assertion failed due to requirement '__is_trivially_copyable(W)'"
//@ diag: 85:15 note "'W' is not trivially copyable"
//@ diag: 85:15 note "because it has a non-trivially-copyable member 'n' of type 'NT'"
//@ diag: 85:15 note "because it has a user-provided destructor"
//@ diag: 75:8 note "'W' defined here"
//@ diag: 86:15 error "static assertion failed due to requirement '__is_trivially_copyable(MV)'"
//@ diag: 86:15 note "'MV' is not trivially copyable"
//@ diag: 86:15 note "because it has a user provided move constructor"
//@ diag: 76:8 note "'MV' defined here"
//@ diag: 87:15 error "static assertion failed due to requirement '__is_trivially_copyable(MA)'"
//@ diag: 87:15 note "'MA' is not trivially copyable"
//@ diag: 87:15 note "because it has a user provided move assignment operator"
//@ diag: 77:8 note "'MA' defined here"
//@ diag: 88:15 error "static assertion failed due to requirement '__is_trivially_copyable(CA)': msg"
//@ diag: 88:15 note "'CA' is not trivially copyable"
//@ diag: 88:15 note "because it has a user provided copy assignment operator"
//@ diag: 78:8 note "'CA' defined here"
//@ diag: 89:15 error "static assertion failed due to requirement '__is_trivially_copyable(U)'"
//@ diag: 89:15 note "'U' is not trivially copyable"
//@ diag: 89:15 note "because it has a non-trivially-copyable member 'n' of type 'NT'"
//@ diag: 79:7 note "'U' defined here"
//@ diag: 90:15 error "static assertion failed due to requirement '__is_trivially_copyable(TS<NT>)'"
//@ diag: 90:15 note "'TS<NT>' is not trivially copyable"
//@ diag: 90:15 note "because it has a non-trivially-copyable member 't' of type 'NT'"
//@ diag: 80:26 note "'TS<NT>' defined here"
//@ diag: 91:16 error "static assertion failed due to requirement '__is_trivially_copyable(const NT)'"
//@ diag: 91:16 note "'const A' (aka 'const NT') is not trivially copyable"
//@ diag: 91:16 note "because it has a user provided copy constructor"
//@ diag: 72:8 note "'NT' defined here"
//@ diag: 92:15 error "static assertion failed due to requirement '__is_trivially_copyable(int &)'"
//@ diag: 92:15 note "'int &' is not trivially copyable"
//@ diag: 92:15 note "because it is a reference type"
//@ diag: 93:15 error "static assertion failed due to requirement '__is_trivially_copyable(NT[2])'"
//@ diag: 93:15 note "'NT[2]' is not trivially copyable"
//@ diag: 94:15 error "static assertion failed due to requirement '__is_trivially_copyable(void)'"
//@ diag: 94:15 note "'void' is not trivially copyable"
//@ diag: 96:20 error "static assertion failed due to requirement '__is_trivially_copyable(V)'"
//@ diag: 96:20 note "'V' is not trivially copyable"
//@ diag: 73:8 note "'V' defined here"
//@ diag: 97:15 error "static assertion failed due to requirement '!__is_trivially_copyable(int)'"
//@ diag: 98:44 error "static assertion failed due to requirement '__is_trivially_copyable(NT)'"
//@ diag: 99:15 note "in instantiation of function template specialization 'f<NT>' requested here"
//@ diag: 98:44 note "'NT' is not trivially copyable"
//@ diag: 98:44 note "because it has a user provided copy constructor"
//@ diag: 72:8 note "'NT' defined here"
namespace std {
template<class T> struct is_trivially_copyable { static constexpr bool value = __is_trivially_copyable(T); };
template<class T> constexpr bool is_trivially_copyable_v = __is_trivially_copyable(T);
}
struct NT { NT(const NT &); };
struct V { virtual void f(); };
struct B : virtual V {};
struct W { virtual void f(); NT n; ~W(); };
struct MV { MV(MV&&); };
struct MA { MA& operator=(MA&&); };
struct CA { CA& operator=(const CA&); };
union U { NT n; };
template<class T> struct TS { T t; };
using A = NT;
static_assert(std::is_trivially_copyable_v<NT>);
static_assert(std::is_trivially_copyable<NT>::value);
static_assert(__is_trivially_copyable(B));
static_assert(__is_trivially_copyable(W));
static_assert(__is_trivially_copyable(MV));
static_assert(__is_trivially_copyable(MA));
static_assert(__is_trivially_copyable(CA), "msg");
static_assert(__is_trivially_copyable(U));
static_assert(__is_trivially_copyable(TS<NT>));
static_assert((__is_trivially_copyable(const A)));
static_assert(__is_trivially_copyable(int &));
static_assert(__is_trivially_copyable(NT[2]));
static_assert(__is_trivially_copyable(void));
constexpr bool t = true;
static_assert(t && __is_trivially_copyable(V));
static_assert(!__is_trivially_copyable(int));
template<class T> void f() { static_assert(__is_trivially_copyable(T)); }
template void f<NT>();
