//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:25 error "'std::initializer_list<int>' layout not recognized. Must be a non-polymorphic class type with no bases and two fields: a 'const E *' and either another 'const E *' or a 'std::size_t'"
//@ diag: 15:25 error "'std::initializer_list<int>' layout not recognized"
//@ diag: 15:25 error "'std::initializer_list<long>' layout not recognized"
//@ diag: 22:25 error "'std::initializer_list<int *>' layout not recognized"
//@ diag: 28:18 error "'std::initializer_list<char>' layout not recognized"
// [dcl.init.list]/5, [support.initlist]/1: list-initializing a
// std::initializer_list binds it to the backing array; an implementation's
// class must have a layout the binding can fill (clang's check, made at
// each such initialization, with the error at the class). Found while
// fixing constant evaluation of std::initializer_list objects.
namespace std {
struct B {};
template<class E> class initializer_list {
  const E *b;
  int n; // not std::size_t
public:
  constexpr int size() const { return n; }
};
// A polymorphic class.
template<class E> class initializer_list<E *> {
  virtual void f();
  const E **b;
  decltype(sizeof 0) n;
};
// A base class.
template<> class initializer_list<char> : B {
  const char *b;
  decltype(sizeof 0) n;
};
}

int g(std::initializer_list<int>);
int a = g({1, 2});
int b = g({1, 2, 3});
void h(std::initializer_list<long>);
void k() { h({1L}); }
void g2(std::initializer_list<int *>);
void k2() { g2({nullptr}); }
void g3(std::initializer_list<char>);
void k3() { g3({'a'}); }
