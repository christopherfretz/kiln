// [cpp.subst]/3, [cpp.stringize], [cpp.concat]: `#__VA_OPT__( )` with a
// paste inside: the stringized token's row lists the tokens stringized,
// the pasted one among them (its own row lists its operands); with no
// variadic argument it lists none. `x ## __VA_OPT__(...)` pastes `x` with
// the first token of the replacement; with no variadic argument the
// placemarker makes no row (spec/pp.md "Preprocessing record", made
// tokens).
//@ expect: compile-ok
//@ strict-diags
//@ pp-record: va-opt-made.record
#define VS(...) #__VA_OPT__(a ## __VA_ARGS__ b)
#define VP(x, ...) x ## __VA_OPT__(__VA_ARGS__)
const char *s1 = VS(1 2);
const char *s2 = VS();
int VP(n, m);
int VP(o);
