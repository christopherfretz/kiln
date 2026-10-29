class K { int x; char y; };
union alignas(4) U { char c; };
struct S { K k; U u; };
