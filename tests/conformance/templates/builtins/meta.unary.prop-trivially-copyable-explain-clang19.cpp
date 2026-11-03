// [meta.unary.prop] is_trivially_copyable: clang 19 explains nothing
// (clang 22: meta.unary.prop-trivially-copyable-explain.cpp).
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang<22
//@ diag: 37:15 error "static assertion failed due to requirement 'std::is_trivially_copyable_v<NT>'"
//@ diag: 38:15 error "static assertion failed due to requirement 'std::is_trivially_copyable<NT>::value'"
//@ diag: 39:15 error "static assertion failed due to requirement '__is_trivially_copyable(B)'"
//@ diag: 40:15 error "static assertion failed due to requirement '__is_trivially_copyable(W)'"
//@ diag: 41:15 error "static assertion failed due to requirement '__is_trivially_copyable(MV)'"
//@ diag: 42:15 error "static assertion failed due to requirement '__is_trivially_copyable(MA)'"
//@ diag: 43:15 error "static assertion failed due to requirement '__is_trivially_copyable(CA)': msg"
//@ diag: 44:15 error "static assertion failed due to requirement '__is_trivially_copyable(U)'"
//@ diag: 45:15 error "static assertion failed due to requirement '__is_trivially_copyable(TS<NT>)'"
//@ diag: 46:16 error "static assertion failed due to requirement '__is_trivially_copyable(const NT)'"
//@ diag: 47:15 error "static assertion failed due to requirement '__is_trivially_copyable(int &)'"
//@ diag: 48:15 error "static assertion failed due to requirement '__is_trivially_copyable(NT[2])'"
//@ diag: 49:15 error "static assertion failed due to requirement '__is_trivially_copyable(void)'"
//@ diag: 51:20 error "static assertion failed due to requirement '__is_trivially_copyable(V)'"
//@ diag: 52:15 error "static assertion failed due to requirement '!__is_trivially_copyable(int)'"
//@ diag: 53:44 error "static assertion failed due to requirement '__is_trivially_copyable(NT)'"
//@ diag: 54:15 note "in instantiation of function template specialization 'f<NT>' requested here"
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
