// [class.friend]/4: a friend declaration may name a constructor of another
// class, here of a nested class whose parameter names an enclosing class
// (`F::I::R::R(F::I::R &&)`).
//@ expect: compile-ok
class F {
  struct I {
    struct R { R(I::R &&); R(int); };
  };
  friend class G;
};
F::I::R::R(I::R &&) {}
F::I::R::R(int) {}
struct S { S(int); };
class G {
  friend F::I::R::R(F::I::R &&);
  friend S::S(int);
};
