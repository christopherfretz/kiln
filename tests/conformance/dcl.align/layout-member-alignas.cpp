// [dcl.align]/1, /5, [class.mem]: an alignment-specifier on a non-static
// data member raises that member's alignment, and so its offset and the
// class's alignment and size (Itanium C++ ABI §2.4). (Class template
// specializations are covered by member-alignas.cpp: the dump does not
// print template arguments yet.)
//@ expect: layout
struct A { char c; alignas(8) char m; char n; };
struct B { char c; char m alignas(16); short s; };
struct C { alignas(4) char a; alignas(double) char b; alignas(2) char arr[3]; };
struct D : A { alignas(32) int i; };
A a; B b; C c; D d;
