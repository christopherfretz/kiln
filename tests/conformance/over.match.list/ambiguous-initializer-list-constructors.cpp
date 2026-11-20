// [over.match.list]/1.1, [over.match.best.general]/3: when two
// initializer-list constructors are viable and neither is better, the
// list-initialization is ambiguous: "call to constructor of 'B' is
// ambiguous" at the initialized entity, the candidates as notes; a
// braced argument of a call makes the call's candidate not viable.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 30:3 error "call to constructor of 'B' is ambiguous"
//@ diag: 27:3 note "candidate constructor"
//@ diag: 28:3 note "candidate constructor"
//@ diag: 31:3 error "call to constructor of 'B' is ambiguous"
//@ diag: 27:3 note "candidate constructor"
//@ diag: 28:3 note "candidate constructor"
//@ diag: 33:14 error "call to constructor of 'B' is ambiguous"
//@ diag: 27:3 note "candidate constructor"
//@ diag: 28:3 note "candidate constructor"
//@ diag: 32:9 note "passing argument to parameter here"
namespace std {
template<class E> class initializer_list {
  const E* b; decltype(sizeof 0) n;
public:
  constexpr initializer_list() : b(nullptr), n(0) {}
  constexpr decltype(sizeof 0) size() const { return n; }
};
}
struct B {
  B(std::initializer_list<int>, int = 0);
  B(std::initializer_list<int>, long = 0);
};
B c{1, 2};
B d = {1};
void f(B);
void g() { f({1, 2}); }
