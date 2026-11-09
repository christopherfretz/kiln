// [dcl.spec.auto.general]: GNU '__auto_type' deduces a variable's type like
// 'auto' (clang `TST_auto_type`, AutoTypeKeyword::GNUAutoType), but it is no
// C++ placeholder: never a return type, parameter, member, 'new' type,
// template argument or typedef, and a structured binding rejects it (WP-1134).
//@ expect: compile-fail
//@ std: c++17
//@ strict-diags
//@ diag: 20:1 error "'__auto_type' not allowed in function return type"
//@ diag: 21:9 error "'__auto_type' not allowed in function prototype"
//@ diag: 22:12 error "'__auto_type' not allowed in non-static struct member"
//@ diag: 24:4 error "'__auto_type' not allowed in template argument"
//@ diag: 26:3 error "'__auto_type' deduced as 'int' in declaration of 'a' and deduced as 'double' in declaration of 'b'"
//@ diag: 27:15 error "declaration cannot be declared with type '__auto_type'; declared type must be 'auto' or reference to 'auto'"
//@ diag: 28:15 error "'__auto_type' not allowed in lambda parameter"
//@ diag: 29:24 error "'__auto_type' not allowed in type allocated by 'new'"
//@ diag: 30:11 error "'__auto_type' not allowed in typedef"
//@ diag: 31:15 error "declaration of variable 'z' with deduced type '__auto_type' requires an initializer"
//@ diag: 32:16 error "'__auto_type' not allowed here"
struct P { int a, b; };
__auto_type f1() { return 1; }
void f2(__auto_type x);
struct M { __auto_type m = 1; static constexpr __auto_type sm = 2; };
template<class T> struct TT {};
TT<__auto_type> tt;
void g() {
  __auto_type a = 1, b = 2.0;
  __auto_type [x, y] = P{1, 2};
  auto l = [](__auto_type q) { return q; };
  __auto_type *n = new __auto_type(1);
  typedef __auto_type TD;
  __auto_type z;
  (void)sizeof(__auto_type);
}
