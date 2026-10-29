struct A { char a; };
struct B { A a; short b; };
struct C { B b[2]; A a; };
struct D { C c; B b; char d; };
