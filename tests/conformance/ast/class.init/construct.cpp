//@ expect: ast
//@ ast-filter: f
// [dcl.init.general]/16.6.2, [dcl.init.list]/3.7, [expr.type.conv]/2,
// [expr.static.cast]/4, [over.match.copy]: initialization of a class by a
// constructor. Direct- and list-initialization call the selected
// constructor (`list` for a braced list); `T(a, b)` and `T{...}` make a
// CXXTemporaryObjectExpr, bound when the destructor is non-trivial; a
// copy-initialization from a non-class value converts by a converting
// constructor (ConstructorConversion); an explicit cast to a class is a
// ConstructorConversion of a constructor call. Matches clang 19.
struct A { int x, y; ~A(); };
struct S { S(int); S(int, int); ~S(); };
struct E { explicit E(int); E(long, long); };
struct V { V(); V(int); };
void take(S);
S give() { return 1; }
void f() {
  A a = A{1, 2};
  S s{1};
  S t = {1};
  S u{1, 2};
  S c = 'c';
  V v{};
  V w = V{};
  V x = {};
  E e{1, 2};
  E g = {1L, 2L};
  (void)S(1, 2);
  (void)(S)3;
  (void)static_cast<V>(4);
  take(5);
}
