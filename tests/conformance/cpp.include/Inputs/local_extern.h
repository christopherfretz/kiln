// Block-scope function and `extern` declarations ([basic.link]/6): each
// names an entity of the enclosing namespace that a later namespace-scope
// declaration redeclares (pch-local-extern-*.cpp, WP-1293).
#ifndef LOCAL_EXTERN_H
#define LOCAL_EXTERN_H
int le_defined(int);
namespace le {
inline int use_fn() { int le_fn(int = 4); return le_fn(); }
inline int use_var() { extern int le_var; return le_var; }
struct S {
  int m() { int le_member(void); extern int le_var; return le_member() + le_var; }
};
// Two block scopes declare the same entity.
inline int twice() { extern int le_var; { int le_fn(int); return le_fn(1) + le_var; } }
inline namespace in {
inline int use_in() { int le_inner(void); return le_inner(); }
}
}
namespace le {
// Already declared at namespace scope: the block-scope declaration
// redeclares it.
inline int use_decl() { int le_decl(void); return le_decl(); }
int le_decl(void);
}
inline int use_global() { int le_defined(int); extern long le_global; return le_defined(2) + int(le_global); }
#endif
