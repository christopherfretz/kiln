struct A { char c; alignas(16) int i; char d; };
struct B { alignas(8) char c; char d; };
struct C { alignas(32) char c[3]; };
