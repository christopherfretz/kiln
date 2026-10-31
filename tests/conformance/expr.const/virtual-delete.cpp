// [expr.delete]/3, [class.dtor]/16: delete through a pointer to a base
// class destroys and frees the most derived object when the destructor
// is virtual; otherwise (and for an object of an array new) it is
// undefined behavior, so not a constant expression (clang's notes).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 19:16 error "constexpr variable 'x1' must be initialized by a constant expression"
//@ diag: 18:46 note "delete of object with dynamic type 'B' through pointer to base class type 'A' with non-virtual destructor"
//@ diag: 19:21 note "in call to 'non_virtual()'"
//@ diag: 46:16 error "constexpr variable 'x2' must be initialized by a constant expression"
//@ diag: 45:46 note "non-array delete used to delete pointer to array object of type 'W[2]'"
//@ diag: 46:21 note "in call to 'array_elem()'"
//@ diag: 44:30 note "heap allocation performed here"
struct A { int a = 1; constexpr ~A() {} };
struct B : A { int b = 2; };

// [expr.delete]/3: deleting through a base without a virtual destructor.
constexpr bool non_virtual() { A *p = new B; delete p; return true; }
constexpr bool x1 = non_virtual();

struct V { constexpr virtual ~V() {} };
struct W : V {};
struct X : W {
  int *log;
  constexpr X(int *l) : log(l) {}
  constexpr ~X() { *log += 1; }
};

// A virtual destructor runs the most derived class's (implicitly
// defined destructors included), then frees the complete object.
constexpr int through_base() {
  int n = 0;
  V *p = new X(&n);
  delete p;
  W *q = new W;
  delete q;
  V *z = nullptr;
  delete z;
  return n;
}
static_assert(through_base() == 1);

// A base subobject of an array element is not a single-object new.
constexpr V *make() { return new W[2]; }
constexpr bool array_elem() { V *p = make(); delete p; return true; }
constexpr bool x2 = array_elem();
