// [over.match.oper]: unary operators, the call operator and the enum-only
// rule for non-member candidates.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:3 error "invalid argument type 'S' to unary expression"
struct S { S& operator++(); S operator++(int); int operator*() const; bool operator!() const; static int operator()(int); };
int operator-(S);
enum E { X };
E operator~(E);
enum class F { Y };
F operator|(F, F);
void f(S s, E e, F x) {
  ++s; s++; *s; !s; -s; s(1); ~e; x | x;
  +s;
}
