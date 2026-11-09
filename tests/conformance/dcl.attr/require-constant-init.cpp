// clang's `require_constant_initialization` attribute (WP-954; clang
// `ConstInitAttr`, the attribute form that predates `constinit`;
// [basic.start.static]/2, [dcl.attr.grammar]/5): it applies to a variable
// with static or thread storage duration, else `only applies to global
// variables`; the initializing declaration must have a constant
// initializer, else an error at the variable, a note at the attribute and
// the evaluation's notes (as for `constinit`, [dcl.constinit]). Added after
// the initializing declaration it is a warning and is dropped (x1).
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 63:10 error "variable does not have a constant initializer"
//@ diag: 63:1 note "required by 'require_constant_initialization' attribute here"
//@ diag: 57:29 note "expanded from macro 'ATTR'"
//@ diag: 63:14 note "non-constexpr function 'f' cannot be used in a constant expression"
//@ diag: 58:5 note "declared here"
//@ diag: 65:11 error "variable does not have a constant initializer"
//@ diag: 65:1 note "required by 'require_constant_initialization' attribute here"
//@ diag: 57:29 note "expanded from macro 'ATTR'"
//@ diag: 65:11 note "non-constexpr constructor 'NonC' cannot be used in a constant expression"
//@ diag: 60:15 note "declared here"
//@ diag: 66:48 error "variable does not have a constant initializer"
//@ diag: 66:3 note "required by 'require_constant_initialization' attribute here"
//@ diag: 66:52 note "non-constexpr function 'f' cannot be used in a constant expression"
//@ diag: 58:5 note "declared here"
//@ diag: 67:23 error "variable does not have a constant initializer"
//@ diag: 67:1 note "required by 'require_constant_initialization' attribute here"
//@ diag: 57:29 note "expanded from macro 'ATTR'"
//@ diag: 67:27 note "non-constexpr function 'f' cannot be used in a constant expression"
//@ diag: 58:5 note "declared here"
//@ diag: 73:19 error "variable does not have a constant initializer"
//@ diag: 73:3 note "required by 'require_constant_initialization' attribute here"
//@ diag: 57:29 note "expanded from macro 'ATTR'"
//@ diag: 73:24 note "non-constexpr function 'f' cannot be used in a constant expression"
//@ diag: 58:5 note "declared here"
//@ diag: 74:25 error "variable does not have a constant initializer"
//@ diag: 74:3 note "required by 'require_constant_initialization' attribute here"
//@ diag: 57:29 note "expanded from macro 'ATTR'"
//@ diag: 74:29 note "non-constexpr function 'f' cannot be used in a constant expression"
//@ diag: 58:5 note "declared here"
//@ diag: 75:3 error "'require_constant_initialization' attribute only applies to global variables"
//@ diag: 57:29 note "expanded from macro 'ATTR'"
//@ diag: 77:12 error "'require_constant_initialization' attribute only applies to global variables"
//@ diag: 57:29 note "expanded from macro 'ATTR'"
//@ diag: 79:3 error "'require_constant_initialization' attribute only applies to global variables"
//@ diag: 57:29 note "expanded from macro 'ATTR'"
//@ diag: 82:1 error "'require_constant_initialization' attribute only applies to global variables"
//@ diag: 57:29 note "expanded from macro 'ATTR'"
//@ diag: 85:10 error "variable does not have a constant initializer"
//@ diag: 85:1 note "required by 'require_constant_initialization' attribute here"
//@ diag: 57:29 note "expanded from macro 'ATTR'"
//@ diag: 85:17 note "non-constexpr function 'f' cannot be used in a constant expression"
//@ diag: 58:5 note "declared here"
//@ diag: 88:8 warning "'require_constant_initialization' attribute added after initialization of variable [-Wignored-attributes]"
//@ diag: 57:29 note "expanded from macro 'ATTR'"
//@ diag: 87:5 note "add the 'require_constant_initialization' attribute to the initializing declaration here"
#define ATTR __attribute__((require_constant_initialization))
int f() { return 1; }
struct Lit { constexpr Lit(int v) : v(v) {} int v; };
struct NonC { NonC(int v) : v(v) {} int v; };

ATTR int a = 42;
ATTR int b = f();
ATTR Lit c(1);
ATTR NonC d(1);
[[clang::require_constant_initialization]] int e = f();
ATTR thread_local int g = f();
ATTR static const int &r = a;
ATTR const int &r2 = 3;

void local() {
  ATTR static int s = 1;
  ATTR static int s2 = f();
  ATTR thread_local int t = f();
  ATTR int automatic = 1;
}
void param(ATTR int p);
struct M {
  ATTR int field;
  ATTR static int sm;
};
ATTR void fn();

extern int late;
ATTR int late = f();
extern int x1;
int x1 = 1;
extern ATTR int x1;
