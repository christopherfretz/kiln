// GCC __builtin_clear_padding takes one pointer to a complete,
// non-const, trivially copyable object type and returns void; GCC checks
// in that order (its wording: "argument 1 in call to function
// '__builtin_clear_padding' ..."). A volatile object, an array and a
// pointer to a pointer are accepted. GCC rejects the same calls.
//@ expect: compile-fail
//@ skip(clang): clang has no __builtin_clear_padding (LLVM issue 64830)
//@ oracle: gcc
//@ strict-diags
//@ diag: 29:27 error "argument 1 in call to function '__builtin_clear_padding' does not have pointer type"
//@ diag: 30:27 error "argument 1 in call to function '__builtin_clear_padding' has pointer to 'const' type ('const S *')"
//@ diag: 31:27 error "argument 1 in call to function '__builtin_clear_padding' points to incomplete type ('void *')"
//@ diag: 32:27 error "argument 1 in call to function '__builtin_clear_padding' points to incomplete type ('Inc *')"
//@ diag: 33:27 error "argument 1 in call to function '__builtin_clear_padding' points to incomplete type ('int (*)[]')"
//@ diag: 34:27 error "argument 1 in call to function '__builtin_clear_padding' has pointer to a non-trivially-copyable type ('NT *')"
//@ diag: 35:27 error "too few arguments to function call, expected 1, have 0"
//@ diag: 36:30 error "too many arguments to function call, expected 1, have 2"
struct S { char c; int i; };
struct NT { NT(const NT &); char c; int i; };
struct Inc;
void ok(S *s, volatile S *vs, S (*a)[4], int **pp) {
  __builtin_clear_padding(s);
  __builtin_clear_padding(vs);
  __builtin_clear_padding(a);
  __builtin_clear_padding(pp);
  static_assert(__is_same(decltype(__builtin_clear_padding(s)), void));
}
void bad(int i, const S *cs, void *vp, Inc *ip, int (*ia)[], NT *nt, S *s) {
  __builtin_clear_padding(i);
  __builtin_clear_padding(cs);
  __builtin_clear_padding(vp);
  __builtin_clear_padding(ip);
  __builtin_clear_padding(ia);
  __builtin_clear_padding(nt);
  __builtin_clear_padding();
  __builtin_clear_padding(s, s);
}
