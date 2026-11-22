export module imgoper;
export namespace on {
struct S { int v; };
// Found by argument-dependent lookup only.
int operator*(S a, long) { return a.v + 10; }
}
// Found by the unqualified lookup at the templates' definitions only:
// declared at global scope, not in an associated namespace of `on::S`,
// and not exported.
int operator+(on::S a, on::S b) { return a.v + b.v; }
int operator-(on::S a, long) { return a.v + 100; }
int operator~(const on::S &a) { return a.v + 1000; }
bool operator==(on::S, long) { return true; }
export template <class X> int add(X a, X b) { return a + b; }
export template <class X> int sub(X a) { return a - 1; }
export template <class X> int cpl(X a) { return ~a; }
export template <class X> bool ne(X a) { return a != 1; }
// The definition's lookup finds no `operator*`: the instantiation has
// argument-dependent lookup only.
export template <class X> int mul(X a) { return a * 1; }
// Declared after the definitions: better matches that the
// instantiations must not see ([temp.dep.candidate]/1).
int operator-(on::S a, int) { return a.v + 200; }
int operator~(on::S &a) { return a.v + 2000; }
bool operator==(on::S, int) { return false; }
int operator*(on::S a, int) { return a.v + 20; }
