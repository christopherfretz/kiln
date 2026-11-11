// C17 6.2.3p1: tags (struct, union, enum) are a name space of their own,
// so `struct S` declares no ordinary identifier `S`: an object, function
// or typedef may be called S too. 6.2.1p4: a struct declared inside
// another struct's member list has the enclosing scope.
//@ expect: compile-ok
//@ strict-diags
struct S { int a; };
int S = 3;
typedef struct T { int b; } T;
T t;
struct T t2;
enum E { E0 } E;
union U { int u; } U;
struct Outer {
  struct Inner { int i; } in;
  int n;
};
struct Inner inner_at_file_scope;
int f(void) { struct S S; S.a = 1; return S.a; }
_Static_assert(sizeof(struct S) == sizeof(int), "");
