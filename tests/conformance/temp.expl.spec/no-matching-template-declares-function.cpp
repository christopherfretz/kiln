// [temp.expl.spec]/1, [temp.deduct.decl]: an explicit specialization that
// matches no function template is reported ("no function template matches
// function template specialization"). When no declaration of the name
// precedes it, clang declares a plain, invalid function instead, so a later
// call resolves to it with no further error (overload resolution still
// reports a call it cannot match, and the result of the call is not
// checked); a redeclaration with another return type is not reported. A
// specialization that names a primary template is not callable as such: the
// call is resolved against the template alone.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 27:18 error "no function template matches function template specialization 'nott'"
//@ diag: 28:17 error "no function template matches function template specialization 'nott2'"
//@ diag: 32:18 error "no function template matches function template specialization 'm1'"
//@ diag: 35:27 error "no function template matches function template specialization 'c1'"
//@ diag: 37:29 error "no function template matches function template specialization 'inner'"
//@ diag: 39:18 error "no function template matches function template specialization 'f'"
//@ diag: 38:25 note "candidate template ignored: failed template argument deduction"
//@ diag: 41:21 error "no function template matches function template specialization 'm'"
//@ diag: 40:36 note "candidate template ignored: failed template argument deduction"
//@ diag: 44:3 error "no matching function for call to 'nott'"
//@ diag: 27:18 note "candidate function not viable: requires 1 argument, but 2 were provided"
//@ diag: 48:3 error "no matching function for call to 'f'"
//@ diag: 38:25 note "candidate function template not viable: requires 1 argument, but 2 were provided"
//@ diag: 49:7 error "no matching member function for call to 'm'"
//@ diag: 40:36 note "candidate function template not viable: requires 1 argument, but 2 were provided"
template <> void nott(int);
template <> int nott2(int x) { return x; }
int nott2(char);
void nott2(int);
namespace M {
template <> void m1(char);
void use() { m1('a'); M::m1(1); }
}
template <> constexpr int c1(int x) { return x; }
static_assert(c1(1) == 1);
struct S { template <> void inner(int); };
template <class T> void f(T);
template <> void f(int, int);
struct X { template <class T> void m(T); };
template <> void X::m(int, int);
void use() {
  nott(1);
  nott(1, 2);
  int n = nott(1);
  nott2(1);
  S().inner(0);
  f(1, 2);
  X().m(1, 2);
}
