// clang's `enable_if` attribute (clang AttrDocs "enable_if"; [dcl.attr.deprecated]/2):
// a `[[deprecated]]` overload distinguished only by its `enable_if` conditions
// warns only where overload resolution selects it.
//@ expect: compile-ok
//@ strict-diags
//@ diag: 18:9 warning "'f' is deprecated: use g"
//@ diag: 11:3 note "'f' has been explicitly marked deprecated here"
//@ diag: 20:3 warning "'S' is deprecated: no"
//@ diag: 14:5 note "'S' has been explicitly marked deprecated here"
constexpr int f(int n) { return n; }
[[deprecated("use g")]] constexpr int f(int n)
    __attribute__((enable_if(n == 1, "one"))) { return 10; }
struct S {
  [[deprecated("no")]] S(int n) __attribute__((enable_if(n == 2, "two"))) {}
  S(int) {}
};
int a = f(2);
int b = f(1);
S s1(3);
S s2(2);
