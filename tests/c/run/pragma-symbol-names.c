// C23 6.10.8 (pragma directive), GNU: '#pragma weak alias = target'
// makes 'alias' a weak alias of the function or object 'target', declared
// before or after the pragma; '#pragma redefine_extname old new' gives
// 'old' the assembly name 'new' (in C every declaration has C linkage);
// '#pragma clang attribute' applies an attribute to the functions it
// selects (clang ActOnPragmaWeakAlias, ActOnPragmaRedefineExtname,
// AddPragmaAttributes). WP-1399.
//@ expect: run
//@ stdout: 3 4 9 2 2 1
#include <stdio.h>
int tgt_fn(void) { return 3; }
#pragma weak alias_fn = tgt_fn
#pragma weak alias_late = late_fn
int late_fn(void) { return 4; }
int tgt_var = 9;
#pragma weak alias_var = tgt_var
#pragma redefine_extname later_fn renamed_later
int later_fn(void) { return 2; }
int renamed_later(void);
#pragma clang attribute push (__attribute__((annotate("c"), cold)), apply_to = function)
static int cold_fn(void) { return 1; }
#pragma clang attribute pop
int main(void) {
    printf("%d %d %d %d %d %d\n", alias_fn(), alias_late(), alias_var, later_fn(), renamed_later(),
           cold_fn());
    return 0;
}
