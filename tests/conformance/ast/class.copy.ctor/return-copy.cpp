//@ expect: ast
//@ ast-filter: ret
// [stmt.return]/2, [class.copy.elision]/3: a returned object is
// copy-initialized; a returned local or parameter is first treated as an
// xvalue (the move constructor is selected), a global is copied, and a
// prvalue initializes the result directly. Matches clang 19.
namespace q {
struct S { int a; ~S(); };
struct T { int a; };
S a0;
S mk();
S ret1(S a) { return a0; }
S ret2(S a) { return a; }
T ret3(T a) { return a; }
S ret4() { return mk(); }
}
