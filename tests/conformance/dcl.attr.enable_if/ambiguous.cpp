// clang's `enable_if` attribute (clang AttrDocs "enable_if",
// `compareEnableIfAttrs`; [over.match.best]/3 as clang extends it): of two
// viable candidates with the same conversions, neither is better when
// neither's conditions are a prefix of the other's, so the call is
// ambiguous.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 20:9 error "call to 'f' is ambiguous"
//@ diag: 14:5 note "candidate function"
//@ diag: 15:5 note "candidate function"
//@ diag: 21:3 error "call to constructor of 'S' is ambiguous"
//@ diag: 17:3 note "candidate constructor"
//@ diag: 18:3 note "candidate constructor"
int f(int n) __attribute__((enable_if(n > 0, "pos"))) { return 1; }
int f(int n) __attribute__((enable_if(n < 10, "small"))) { return 2; }
struct S {
  S(int n) __attribute__((enable_if(n > 0, "pos"))) {}
  S(int n) __attribute__((enable_if(n < 10, "small"))) {}
};
int a = f(5);
S s(5);
