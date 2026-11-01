// DR1467 (WP-170): a braced list holding one element of the class or a
// derived class initializes the object from that element (an elided
// prvalue, else the copy/move constructor) even when the class has an
// initializer-list constructor that could take the element.
namespace std {
typedef decltype(sizeof 0) size_t;
template<class E> class initializer_list {
  const E *b; size_t n;
  constexpr initializer_list(const E *p, size_t s) : b(p), n(s) {}
public:
  constexpr initializer_list() : b(nullptr), n(0) {}
  constexpr size_t size() const { return n; }
  constexpr const E *begin() const { return b; }
  constexpr const E *end() const { return b + n; }
};
}
struct B { int v; B(std::initializer_list<B>); B(int); B(const B&); B(B&&); };
B make(int);
struct C : B { C(int); };
B f1() { B x{make(1)}; return x; }
B f2(const B &b) { B x{b}; return x; }
B f3(const C &c) { B x{c}; return x; }
B f4() { B x = {make(2)}; return x; }
void g(B);
void f5() { g({make(3)}); }
B f6(B &&b) { return {static_cast<B&&>(b)}; }
B f7(C &&c) { return B{static_cast<C&&>(c)}; }
B f8() { B x{make(1), make(2)}; return x; }
