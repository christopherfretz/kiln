// GNU pragmas naming symbols in C: '#pragma weak alias = target' emits a
// weak alias with the alias's own (unmangled) name; '#pragma
// redefine_extname old new' renames a declaration before (unused) or
// after the pragma; a target never declared is reported at the end of the
// file (clang ActOnPragmaWeakAlias, ActOnPragmaRedefineExtname). WP-1399.
//@ expect: ir
//@ diag: 16:14 warning "weak identifier 'nowhere' never declared"
int tgt(void) { return 1; }
#pragma weak al = tgt
#pragma weak al_late = late
int late(void) { return 2; }
int obj = 3;
#pragma weak al_obj = obj
extern int old_name(void);
#pragma redefine_extname old_name new_name
#pragma weak al_none = nowhere
int use(void) { return al() + al_late() + al_obj + old_name(); }
