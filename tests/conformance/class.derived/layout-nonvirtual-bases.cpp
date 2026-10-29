// [class.derived]/2-4, [class.mem]/19 (later members at higher addresses),
// Itanium C++ ABI §2.4 (non-virtual bases allocated in declaration order,
// then the non-static data members, each at its alignment; dsize/nvsize and
// tail-padding reuse for non-POD bases).
//@ expect: layout
struct A { int a; char c; };              // POD for layout: tail padding kept
struct B { double d; };
struct C : A, B { char x; };              // A at 0, B at 8, x at 16; dsize 17
struct NP { private: int p; protected: char q; };  // not POD: mixed access
struct T : NP { char r; };                // r reuses NP's tail padding
struct M : C { short s; long long l; };   // s at 18, in C's tail padding
M m;
T t;
