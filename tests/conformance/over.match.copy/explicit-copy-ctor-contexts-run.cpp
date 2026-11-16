// [over.match.copy]/1, [over.match.ctor]/1, [dcl.init.general]/15-16: with an
// explicit copy constructor and a converting constructor template, every
// copy-initialization (the `= e` structured binding of an array's elements,
// [dcl.struct.bind]/1; a parenthesized aggregate's elements; a function
// argument; a return value; a thrown object and a handler's parameter; a
// default member initializer; `T x = e;`) uses the template, and every
// direct-initialization (the `(e)` binding, the hidden variable of a
// class binding through the implicit copy constructor, `T x(e)`, `T x{e}`)
// the explicit copy constructor. Found running GCC's C++ testsuite.
// Matches clang 19 and 22; GCC 16.1 counts one explicit copy for the throw.
//@ expect: run
//@ std: c++20
//@ strict-diags
//@ stdout-file: explicit-copy-ctor-contexts-run.stdout
extern "C" int printf(const char*, ...);
namespace std { template<class T> struct tuple_size; template<unsigned long, class T> struct tuple_element; }
int expl, tmpl;
struct A {
  A() {}
  explicit A(const A&) { ++expl; }
  template <class T> A(const T&) { ++tmpl; }
};
struct P { A a, b; };
struct Q { A x; template <int I> const A& get() const { return x; } };
template<> struct std::tuple_size<Q> { static constexpr int value = 2; };
template<unsigned long I> struct std::tuple_element<I, Q> { using type = const A; };
A ga;
struct DM2 { A m = ga; };
A retl() { return ga; }
void take(A) {}
void p(const char* s) { printf("%s %d %d\n", s, expl, tmpl); expl = tmpl = 0; }
int main() {
  A arr[2];
  { auto [x, y] = arr; } p("sb-array-copy");
  { auto [x, y](arr); } p("sb-array-paren");
  Q q;
  { auto [x, y] = q; } p("sb-hidden-copy");
  { auto [x, y](q); } p("sb-hidden-paren");
  { P pp(ga, ga); } p("paren-aggr");
  { take(ga); } p("arg");
  { retl(); } p("return");
  { try { throw ga; } catch (A c) {} } p("throw-catch");
  { DM2 d; } p("dmi");
  { A a = ga; } p("var-copy");
  { A a(ga); } p("var-direct");
  { A a{ga}; } p("var-brace");
}
