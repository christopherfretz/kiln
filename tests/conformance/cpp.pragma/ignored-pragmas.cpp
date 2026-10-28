// [cpp.pragma]/1: pragmas kiln recognizes but does not act on are accepted
// without a diagnostic.
//@ expect: compile-ok
//@ args: -Wunknown-pragmas
//@ strict-diags
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wunused-variable"
#pragma GCC diagnostic push
#pragma GCC diagnostic warning "-Wshadow"
#pragma GCC diagnostic pop
#pragma clang diagnostic pop
int x;
