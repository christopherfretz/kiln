// [class.friend]/4: a friend declaration may name a constructor of another
// class, here of a nested class whose parameter names an enclosing class
// (`F::I::R::R(F::I::R &&)`). kiln does not declare friend constructors yet.
//@ expect: compile-ok
//@ xfail(kiln): WP-1466, a friend declaration naming a constructor is "constructors is not supported yet" (act_on_special_declarator has no class for it)
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
