//@ expect: no-crash
// The kind's own `-ferror-limit=0` reaches GCC as `-fmax-errors=0`.
int f() { return nope; }
