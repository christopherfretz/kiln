// The declaration-extent table (spec/ast.md §19.2): the body range of a
// function (its ctor-initializer and a function-try-block's handlers
// included), of a class, an enumeration and a linkage specification,
// and `= default`, `= delete` (with its reason) and `= 0`.
//@ decl-extents: bodies.extents
//@ std: c++26
struct B {
  int m;
  B() : m(1) {}
  B(int v) try : m(v) {
  } catch (...) {
  }
  B(const B &) = default;
  B &operator=(const B &) = delete("no copies");
  virtual void pure() = 0;
  virtual ~B();
  void late();
};
B::~B() = default;
void B::late() {}
enum class E : int { a, b = 2 };
extern "C++" {
void in_linkage();
}
int main() try {
  return 0;
} catch (...) {
  return 1;
}
