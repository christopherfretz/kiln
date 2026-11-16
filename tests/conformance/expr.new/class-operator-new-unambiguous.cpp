// [expr.new]/12, [class.member.lookup]: an allocation function a derived
// class redeclares, or one found in a single base, is not ambiguous.
//@ std: c++17
struct P { void *operator new(decltype(sizeof 0)); };
struct Q { void *operator new(decltype(sizeof 0)); };
struct R : P, Q { void *operator new(decltype(sizeof 0)); };
struct T : P { int i; };
R *mk() { return new R; }
T *mk2() { return new T; }
R *mk3() { return ::new R; }
