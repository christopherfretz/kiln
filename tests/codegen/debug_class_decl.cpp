// Debug info (WP-1389): under limited -g (the default) a class that this
// unit never requires to be complete (clang's
// `isCompleteDefinitionRequired`, set by `RequireCompleteType`) is a
// DIFlagFwdDecl declaration, even when it is defined here. Ptr is only
// named through a pointer and Alias through a typedef of a pointer; the
// others are required: a member access (Mem), a member
// pointer through a nested-name-specifier (MP), a variable (Var), a call
// of a class object (Fn), an implicit copy assignment (Asg) and every
// closure type. In an explicit instantiation declaration
// with a member defined in the class, Ext<int> is a declaration though
// required (clang's `hasExplicitMemberDefinition`).
//@ args: -g
struct Ptr { int a; };
struct Alias { int a; };
typedef Alias *AliasPtr;
struct Mem { int a; };
struct MP { int y; };
struct Var { int a; };
struct Fn { int operator()() const { return 2; } };
struct Asg { int a; };
template <class T> struct Ext { T v; T get() const { return v; } };
extern template struct Ext<int>;

Ptr *gp;
AliasPtr ga;
int MP::*gm = &MP::y;

int use(Mem *m, Asg *x, Asg *y, Ext<int> *e) {
  *x = *y;
  Var v{1};
  Fn f;
  auto l = [&] { return v.a; };
  return m->a + f() + l() + e->v;
}
