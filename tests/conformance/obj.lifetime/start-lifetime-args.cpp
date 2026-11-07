// [obj.lifetime] (P3726R2): std::start_lifetime(T&) requires T to be a
// complete implicit-lifetime aggregate. GCC's __builtin_start_lifetime
// takes one pointer and checks the pointee in that order: complete type,
// aggregate, implicit-lifetime (GCC's wording, clang's argument-count
// and incomplete-type diagnostics).
//@ skip(clang): clang 22 has no __builtin_start_lifetime
//@ std: c++26
//@ expect: compile-fail
//@ strict-diags
//@ diag: 27:28 error "'__builtin_start_lifetime' argument type 'int' is not pointer type"
//@ diag: 28:28 error "argument type 'int *' is not a pointer to aggregate type"
//@ diag: 29:28 error "argument type 'NA *' is not a pointer to aggregate type"
//@ diag: 30:28 error "argument type 'D *' is not a pointer to implicit-lifetime type"
//@ diag: 31:28 error "incomplete type 'Inc' where a complete type is required"
//@ diag: 20:8 note "forward declaration of 'Inc'"
//@ diag: 32:28 error "too few arguments to function call, expected 1, have 0"
//@ diag: 33:32 error "too many arguments to function call, expected 1, have 2"
struct NA { NA(int); int x; };
struct D { int x; ~D() {} };
struct Inc;
struct A { int x; int y; };
void ok(A *ap, int (*arr)[3]) {
  __builtin_start_lifetime(ap);
  __builtin_start_lifetime(arr);
}
void bad(int *ip, int i, NA *np, D *dp, Inc *incp) {
  __builtin_start_lifetime(i);
  __builtin_start_lifetime(ip);
  __builtin_start_lifetime(np);
  __builtin_start_lifetime(dp);
  __builtin_start_lifetime(incp);
  __builtin_start_lifetime();
  __builtin_start_lifetime(ip, ip);
}
