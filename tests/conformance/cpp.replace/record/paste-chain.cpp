// [cpp.concat]/3: a chain `a ## b ## c` makes one token, whose made row
// lists its three operands left to right, each an argument token at its
// location before substitution. A placemarker on either side of a `##`
// makes no token and no row: the token left keeps its argument or body
// origin (spec/pp.md "Preprocessing record", made tokens).
//@ expect: compile-ok
//@ strict-diags
//@ pp-record: paste-chain.record
#define CAT3(a, b, c) a ## b ## c
#define LEFT(a) a ## _tail
#define RIGHT(a) head_ ## a
int CAT3(x, y, z);
int LEFT();
int RIGHT();
int CAT3(, q, );
