// [meta.const.eval]/4 (P2641R4), [expr.const]/16, [basic.life]/1:
// `__builtin_is_within_lifetime` takes one pointer to an object type (a
// non-pointer or function-pointer argument is an error at the argument);
// a null or one-past-the-end argument makes the evaluation fail with a
// note naming the builtin (or the `std::is_within_lifetime` caller), and
// a pointer to a destroyed, deleted or out-of-scope object is a read
// outside its lifetime. Outside an immediate function context the call is
// an immediate invocation that must be a constant expression.
//@ requires: clang>=22
//@ expect: compile-fail
//@ strict-diags
//@ diag: 50:32 error "non-pointer argument to '__builtin_is_within_lifetime' is not allowed"
//@ diag: 51:32 error "function pointer argument to '__builtin_is_within_lifetime' is not allowed"
//@ diag: 52:32 error "non-pointer argument to '__builtin_is_within_lifetime' is not allowed"
//@ diag: 53:32 error "non-pointer argument to '__builtin_is_within_lifetime' is not allowed"
//@ diag: 54:32 error "too few arguments to function call, expected 1, have 0"
//@ diag: 55:36 error "too many arguments to function call, expected 1, have 2"
//@ diag: 58:15 error "static assertion expression is not an integral constant expression"
//@ diag: 57:54 note "'__builtin_is_within_lifetime' cannot be called with a null pointer"
//@ diag: 58:15 note "in call to 'null_ptr()'"
//@ diag: 60:15 error "static assertion expression is not an integral constant expression"
//@ diag: 59:48 note "'__builtin_is_within_lifetime' cannot be called with a one-past-the-end pointer"
//@ diag: 60:15 note "in call to 'past_end()'"
//@ diag: 62:15 error "static assertion expression is not an integral constant expression"
//@ diag: 61:54 note "'std::is_within_lifetime' cannot be called with a null pointer"
//@ diag: 61:54 note "in call to 'is_within_lifetime<int>(nullptr)'"
//@ diag: 62:15 note "in call to 'std_null()'"
//@ diag: 70:15 error "static assertion expression is not an integral constant expression"
//@ diag: 66:12 note "read of object outside its lifetime is not allowed in a constant expression"
//@ diag: 70:16 note "in call to 'destroyed()'"
//@ diag: 72:15 error "static assertion expression is not an integral constant expression"
//@ diag: 71:63 note "read of heap allocated object that has been deleted"
//@ diag: 72:16 note "in call to 'deleted()'"
//@ diag: 78:15 error "static assertion expression is not an integral constant expression"
//@ diag: 76:10 note "read of object outside its lifetime is not allowed in a constant expression"
//@ diag: 78:16 note "in call to 'dangling()'"
//@ diag: 81:39 error "call to consteval function '__builtin_is_within_lifetime' is not a constant expression"
//@ diag: 81:68 note "function parameter 'p' with unknown value cannot be used in a constant expression"
//@ diag: 81:27 note "declared here"
//@ diag: 82:44 error "call to consteval function '__builtin_is_within_lifetime' is not a constant expression"
//@ diag: 82:44 note "read of non-const variable 'i' is not allowed in a constant expression"
//@ diag: 82:30 note "declared here"
namespace std {
template <class T> consteval bool is_within_lifetime(const T* p) noexcept {
  return __builtin_is_within_lifetime(p);
}
} // namespace std
struct S { int a; };
consteval void bad_args(int i, void (*fp)(), int S::*mp) {
  __builtin_is_within_lifetime(i);
  __builtin_is_within_lifetime(fp);
  __builtin_is_within_lifetime(nullptr);
  __builtin_is_within_lifetime(mp);
  __builtin_is_within_lifetime();
  __builtin_is_within_lifetime(&i, &i);
}
consteval bool null_ptr() { int* p = nullptr; return __builtin_is_within_lifetime(p); }
static_assert(null_ptr());
consteval bool past_end() { int a[2]{}; return __builtin_is_within_lifetime(a + 2); }
static_assert(past_end());
consteval bool std_null() { int* p = nullptr; return std::is_within_lifetime(p); }
static_assert(std_null());
consteval bool destroyed() {
  S* s = new S;
  s->~S();
  bool r = __builtin_is_within_lifetime(s);
  ::operator delete(s);
  return r;
}
static_assert(!destroyed());
consteval bool deleted() { int* p = new int; delete p; return __builtin_is_within_lifetime(p); }
static_assert(!deleted());
consteval bool dangling() {
  int* p;
  { int x = 0; p = &x; }
  return __builtin_is_within_lifetime(p);
}
static_assert(!dangling());
// Outside an immediate function context the call is an immediate
// invocation that must be a constant expression.
constexpr bool param(int* p) { return __builtin_is_within_lifetime(p); }
constexpr bool local() { int i = 0; return __builtin_is_within_lifetime(&i); }
