struct alignas(16) A { char c; };
struct alignas(8) B { int i; char c; };
struct U { A a; char c; };
union alignas(32) X { char c; };
