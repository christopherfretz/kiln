// [temp.constr.atomic]/1, [temp.constr.order]: an atom's parameter mapping
// covers the template parameters its expression names. A member class or
// enumeration of a local class, named in a member function body before its
// declaration (a complete-class context), depends only on what the local
// class's declaration names, so `C<T, int>` and `C<T, long>` give one atom
// and the overload adding a conjunct is more constrained. A member class
// naming `U` keeps the atoms apart (ambiguous). WP-883 (clang 19 oracle).
//@ requires: clang<22.0.0
//@ expect: compile-fail
//@ strict-diags
//@ diag: 47:9 error "call to 'f3' is ambiguous"
//@ diag: 45:54 note "candidate function [with T = long]"
//@ diag: 46:63 note "candidate function [with T = long]"
template <class T, class U> concept M1 = [] {
  struct L {
    static constexpr int f() { return sizeof(N); }
    struct N { T t; };
  };
  return L::f() > 1;
}();
template <class T> requires M1<T, int> constexpr int f1(T) { return 1; }
template <class T> requires M1<T, long> && true constexpr int f1(T) { return 2; }
static_assert(f1(1L) == 2);

template <class T, class U> concept M2 = [] {
  struct L {
    static constexpr int f() { return int(E::e) + int(sizeof(typename O::P)); }
    enum class E { e = sizeof(T) };
    struct O { struct P { T t; }; };
  };
  return L::f() > 1;
}();
template <class T> requires M2<T, int> constexpr int f2(T) { return 1; }
template <class T> requires M2<T, long> && true constexpr int f2(T) { return 2; }
static_assert(f2(1L) == 2);

// the member class names U: the atoms differ
template <class T, class U> concept M3 = [] {
  struct L {
    static constexpr int f() { return sizeof(N); }
    struct N { U u; };
  };
  return L::f() > 1;
}();
template <class T> requires M3<T, int> constexpr int f3(T) { return 1; }
template <class T> requires M3<T, long> && true constexpr int f3(T) { return 2; }
int q = f3(1L); // ambiguous
