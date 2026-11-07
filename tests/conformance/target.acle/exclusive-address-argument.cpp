// [driver] `__builtin_arm_ldrex`/`ldaex` and `__builtin_arm_strex`/`stlex`
// (the arm_acle.h exclusive accesses) take a pointer to an integer,
// floating or pointer type: a load is the pointee, qualifiers kept, a store
// converts its value to it and is `int`; any other address is an error
// naming it as converted, `const volatile T *` for a load and `volatile
// T *` for a store (spec/landed/sema/WP-724.md "ACLE builtins"). WP-724.
//@ requires: target-aarch64
//@ verify
struct S { int a; };
static_assert(__is_same(decltype(__builtin_arm_ldrex((const int *)0)), const int));
static_assert(__is_same(decltype(__builtin_arm_ldaex((double *)0)), double));
static_assert(__is_same(decltype(__builtin_arm_strex((char *)0, (char **)0)), int));
void f(S *s, int i) {
  __builtin_arm_ldrex(s); // expected-error {{address argument to atomic builtin must be a pointer to integer, floating-point or pointer ('const volatile S *' invalid)}}
  __builtin_arm_stlex(*s, s); // expected-error {{address argument to atomic builtin must be a pointer to integer, floating-point or pointer ('volatile S *' invalid)}}
  __builtin_arm_ldaex(i); // expected-error {{address argument to atomic builtin must be a pointer ('int' invalid)}}
}
