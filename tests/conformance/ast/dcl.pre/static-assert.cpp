//@ expect: ast
static_assert(true);
static_assert(1 + 1 == 2, "math");
static_assert(sizeof(char) == 1, "bytes");
