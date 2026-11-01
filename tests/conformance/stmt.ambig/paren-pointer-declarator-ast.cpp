// [stmt.ambig]: `T(*g)(nullptr);` declares `g`, a pointer to T
// direct-initialized with nullptr. clang keeps the redundant parentheses
// as ParenType sugar (`T (*)`).
//@ expect: ast
//@ xfail(kiln): no ParenType sugar; kiln dumps the type as 'T *'
struct T { T(); };
void s11() { T(*g)(nullptr); }
