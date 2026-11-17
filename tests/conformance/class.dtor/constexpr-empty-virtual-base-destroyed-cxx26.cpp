// [class.dtor]/13 (C++26, P3533R2): the destructor of a complete object
// destroys its virtual bases after its members and non-virtual bases, in
// the reverse order of their construction; a destructor called for a
// base class subobject does not. Empty virtual bases included: they have
// no storage of their own, but their destructors run, once per complete
// object, also for array elements, `new` objects (the rejected side), temporaries and objects
// destroyed by unwinding. kiln skipped the destructor of an empty virtual
// base, so a constant destruction looked possible where it was not. GCC
// 16.1 skips it too (`thr()` below fails there); the text decides.
// The rejected side is constexpr-empty-virtual-base-destroyed-cxx26-errors.cpp.
//@ skip(clang): neither clang 19 nor clang 22 implements P3533
//@ expect: compile-ok
//@ strict-diags
//@ std: c++26

// Throws when destroyed: an empty class whose destructor has an effect.
struct Th {
  constexpr ~Th() noexcept(false) { throw 7; }
};
struct T1 : virtual Th {
  int v = 1;
};
constexpr int one() {
  try {
    T1 t;
  } catch (int i) {
    return i;
  }
  return 0;
}
static_assert(one() == 7);

// A diamond: the shared virtual base is destroyed once (a second throw
// during the unwinding would terminate), and not by the base subobjects'
// destructors.
struct A : virtual Th {};
struct B : virtual Th {};
struct D : A, B {
  int v = 2;
};
constexpr int diamond() {
  try {
    D d;
  } catch (int i) {
    return i;
  }
  return 0;
}
static_assert(diamond() == 7);

// The counted members and the counted (non-empty) virtual base are
// destroyed before the empty one throws.
struct Count {
  int *c;
  constexpr ~Count() { ++*c; }
};
struct M : virtual Th, virtual Count {
  Count m;
  constexpr M(int *c) : Count{c}, m{c} {}
};
constexpr int mixed() {
  int n = 0;
  try {
    M x(&n);
  } catch (int i) {
    return i * 10 + n;
  }
  return 0;
}
static_assert(mixed() == 72);

constexpr int temporary() {
  try {
    (void)T1().v;
  } catch (int i) {
    return i;
  }
  return 0;
}
static_assert(temporary() == 7);

// An empty virtual base with an ordinary destructor: no effect, the
// objects are constant-destructible.
struct Quiet {
  constexpr ~Quiet() {}
};
struct Q : virtual Quiet {
  int v = 3;
};
struct QD : Q {};
constexpr Q q;
constexpr QD qd[2];
static_assert(q.v + qd[1].v == 6);
constexpr int arrays() {
  QD a[3];
  Q *p = new Q[2];
  delete[] p;
  return a[2].v;
}
static_assert(arrays() == 3);
