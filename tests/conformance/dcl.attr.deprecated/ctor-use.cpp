// [dcl.attr.deprecated]/2: a use of a deprecated constructor warns where
// overload resolution selects it: at the variable for direct- and list-
// initialization, at the initializer for copy-initialization from a class
// prvalue, at the converted expression for an implicit conversion, and in
// mem-initializers, new-expressions and instantiations (with their notes).
//@ expect: compile-ok
//@ strict-diags
//@ diag: 49:3 warning "'S' is deprecated: no"
//@ diag: 44:5 note "'S' has been explicitly marked deprecated here"
//@ diag: 50:8 warning "'S' is deprecated: no"
//@ diag: 44:5 note "'S' has been explicitly marked deprecated here"
//@ diag: 51:3 warning "'S' is deprecated: no"
//@ diag: 44:5 note "'S' has been explicitly marked deprecated here"
//@ diag: 52:3 warning "'S' is deprecated: no"
//@ diag: 44:5 note "'S' has been explicitly marked deprecated here"
//@ diag: 53:8 warning "'S' is deprecated"
//@ diag: 46:5 note "'S' has been explicitly marked deprecated here"
//@ diag: 54:22 warning "'S' is deprecated: no"
//@ diag: 44:5 note "'S' has been explicitly marked deprecated here"
//@ diag: 57:14 warning "'S' is deprecated: no"
//@ diag: 44:5 note "'S' has been explicitly marked deprecated here"
//@ diag: 57:20 warning "'S' is deprecated"
//@ diag: 46:5 note "'S' has been explicitly marked deprecated here"
//@ diag: 58:12 warning "'S' is deprecated: no"
//@ diag: 44:5 note "'S' has been explicitly marked deprecated here"
//@ diag: 60:14 warning "'S' is deprecated: no"
//@ diag: 44:5 note "'S' has been explicitly marked deprecated here"
//@ diag: 61:18 warning "'S' is deprecated: no"
//@ diag: 44:5 note "'S' has been explicitly marked deprecated here"
//@ diag: 62:13 warning "'S' is deprecated: no"
//@ diag: 44:5 note "'S' has been explicitly marked deprecated here"
//@ diag: 62:16 warning "'S' is deprecated: no"
//@ diag: 44:5 note "'S' has been explicitly marked deprecated here"
//@ diag: 63:18 warning "'S' is deprecated: no"
//@ diag: 44:5 note "'S' has been explicitly marked deprecated here"
//@ diag: 63:24 warning "'S' is deprecated: no"
//@ diag: 44:5 note "'S' has been explicitly marked deprecated here"
//@ diag: 63:35 warning "'S' is deprecated: no"
//@ diag: 44:5 note "'S' has been explicitly marked deprecated here"
//@ diag: 64:31 warning "'S' is deprecated: no"
//@ diag: 65:8 note "in instantiation of function template specialization 'tf<S>' requested here"
//@ diag: 44:5 note "'S' has been explicitly marked deprecated here"
struct S {
  [[deprecated("no")]] S(int n) {}
  S(double) {}
  [[deprecated]] S(const S&) = default;
  S(S&&) = default;
};
S s2(2);
S s3 = S(2);
S s4{2};
S s5 = 2;
S s6 = s5;
struct D : S { D() : S(1) {} };
D d1;
void f(S);
void g() { f(3); f(s5); }
S *p = new S(5);
[[deprecated]] void h() { S x(1); }
const S &r = 2;
S ret() { return 4; }
S arr[2] = {1, 2};
struct M { S m = 1; S n{2}; M() : m(3) {} };
template <class T> T tf() { T t(1); return t; }
S ts = tf<S>();
