//@ expect: ast
//@ ast-filter: f
// [dcl.init.general]/16.6.2.2 (P0960): a parenthesized initialization of
// a union initializes its first member; an array of classes takes the
// default construction as its filler (zeroing unless user-provided); an
// array new-expression's list always carries a filler, and with a
// runtime bound or none its type is sized by the arguments. Matches
// clang 19.
union U { int i; float f; };
struct S { S(); S(int); int v; };
struct T { int v; };
void f(int n) {
  U u(4);
  U w(2.5);
  S s[3](S(1));
  T t[2](T{1});
  (void)new int[3](1, 2);
  (void)new int[n](1, 2);
  (void)new int[](1, 2, 3);
  (void)new S[2](1);
}
