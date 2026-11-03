// GNU `nothrow` (WP-186; GNU [except.spec] extension, clang
// handleFunctionTypeAttr AT_NoThrow / EST_NoThrow): a function type
// attribute applied to the declared function type (through a pointer,
// reference or member pointer too). With no exception specification
// the type is `__attribute__((nothrow))`, non-throwing and the same
// type as `noexcept`; a potentially-throwing written specification
// ignores it with a warning; on a non-function it is rejected. A
// redeclaration may omit it (-Wmissing-exception-spec) and an overrider
// may drop it (-Wmicrosoft-exception-spec), but adding it later is a
// mismatch. Verified against clang 22 and 19.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 38:6 error "cannot initialize a variable of type 'int *' with an rvalue of type 'void (*)() __attribute__((nothrow))'"
//@ diag: 40:41 warning "'nothrow' attribute conflicts with exception specification; attribute ignored"
//@ diag: 45:22 warning "'nothrow' only applies to function types; type here is 'int'"
//@ diag: 45:22 warning "'nothrow' attribute only applies to functions and function pointers"
//@ diag: 47:6 warning "'a' is missing exception specification '__attribute__((nothrow))'"
//@ diag: 26:6 note "previous declaration is here"
//@ diag: 49:6 error "exception specification in declaration does not match previous declaration"
//@ diag: 48:6 note "previous declaration is here"
//@ diag: 50:21 warning "exception specification of overriding function is more lax than base version"
//@ diag: 29:60 note "overridden virtual function is here"
template<class T, class U> struct same { static constexpr bool v = false; };
template<class T> struct same<T, T> { static constexpr bool v = true; };

void a() __attribute__((nothrow));
[[gnu::nothrow]] void b();
__attribute__((nothrow)) void c();
struct S { void m() __attribute__((nothrow)); virtual void v() __attribute__((nothrow)); };
void (*fp)() __attribute__((nothrow));
void (&fr)() __attribute__((nothrow)) = a;
template<class T> void tf() __attribute__((nothrow));
auto lam = []() __attribute__((nothrow)) {};
static_assert(noexcept(a()) && noexcept(b()) && noexcept(c()));
static_assert(noexcept(S().m()) && noexcept(fp()) && noexcept(fr()));
static_assert(noexcept(tf<int>()) && noexcept(lam()));
static_assert(same<decltype(a), void() noexcept>::v);
int *p = &a;

void e() noexcept(false) __attribute__((nothrow));
static_assert(!noexcept(e()));
void g() noexcept __attribute__((nothrow));
void h() __attribute__((nothrow));
void h() noexcept;
int x __attribute__((nothrow));

void a() {}
void k();
void k() __attribute__((nothrow));
struct D : S { void v() override; };
