// clang `handleDeprecatedAttr`: the GNU spelling takes an optional second
// string argument, the replacement a fix-it offers (LLVM's
// `LLVM_DEPRECATED(MSG, FIX)`); the use warns with the message alone.
// The standard and `[[gnu::]]` spellings take at most a message: a second
// one is an error that still applies the attribute; a third, or a
// non-string replacement, drops it.
//@ expect: compile-fail
//@ strict-diags
//@ std: c++17
//@ requires: clang>=22
//@ diag: 25:3 error "'deprecated' attribute takes no more than 1 argument"
//@ diag: 26:3 error "'gnu::deprecated' attribute takes no more than 1 argument"
//@ diag: 27:16 error "'deprecated' attribute takes no more than 2 arguments"
//@ diag: 28:32 error "expected string literal as argument of 'deprecated' attribute"
//@ diag: 30:24 warning "'f' is deprecated: Use g instead"
//@ diag: 22:18 note "'f' has been explicitly marked deprecated here"
//@ diag: 31:10 warning "'s' is deprecated: m"
//@ diag: 25:3 note "'s' has been explicitly marked deprecated here"
//@ diag: 31:16 warning "'u' is deprecated: m"
//@ diag: 26:3 note "'u' has been explicitly marked deprecated here"
struct T {
  __attribute__((deprecated("Use g instead", "g"))) int f() { return 1; }
  int g() { return 2; }
};
[[deprecated("m", "r")]] int s();
[[gnu::deprecated("m", "r")]] int u();
__attribute__((deprecated("m", "r", "x"))) int v();
__attribute__((deprecated("m", 1))) int w();
int h() {
  return T().g() + T().f() +
         s() + u() + v() + w();
}
