// GNU `may_alias` (WP-423; clang `MayAliasAttr`, a simple attribute with no
// subject list): accepted without a diagnostic on any declaration -- a
// typedef, an alias-declaration, a class, an enumeration, a variable, a
// function, a data member, and after a ptr-operator (where clang applies a
// GNU declaration attribute to the declaration); it takes no arguments,
// and a `[[gnu::may_alias]]` at the end of a decl-specifier-seq is no type
// attribute. Codegen puts accesses through such a type in the `omnipotent
// char` alias class (tests/codegen/tbaa_may_alias_o1.cpp).
// [dcl.attr.grammar]/5 (implementation-defined attributes), [basic.lval]/11.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 32:20 error "'may_alias' attribute takes no arguments"
//@ diag: 33:7 error "may_alias' attribute cannot be applied to types"
typedef int __attribute__((may_alias)) mint;
using aint [[gnu::may_alias]] = int;
typedef int* __attribute__((may_alias)) mptr;
struct __attribute__((may_alias)) S { int x; };
struct [[gnu::may_alias]] S2 { int x; };
enum __attribute__((may_alias)) E { A };
template <class T> struct __attribute__((may_alias)) TT { T t; };
int __attribute__((may_alias)) v;
void __attribute__((may_alias)) g();
struct M { int m __attribute__((may_alias)); };
int* __attribute__((may_alias)) q;

static_assert(sizeof(mint) == sizeof(int));
static_assert(__is_same(aint, int) && __is_same(mptr, int*));
static_assert(sizeof(TT<int>) == sizeof(int));

int use(mint* p, S* s, E* e) { return *p + s->x + (*e == A); }

int __attribute__((may_alias(1))) z;
int [[gnu::may_alias]] w;
