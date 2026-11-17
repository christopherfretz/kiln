// [class.friend]/3: a friend type declaration is `friend` followed by a
// type specifier: no other decl-specifier may come first, a class is not
// marked inline or constexpr, and no virtual, storage class or cv-qualifier
// is written. [class.friend]/1: a constructor, destructor or conversion
// function is befriended by its qualified name.
// Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 25:3 error "'inline' can only appear on functions and non-local variables"
//@ diag: 25:10 error "'friend' must appear first in a non-function declaration"
//@ diag: 26:9 error "'friend' must appear first in a non-function declaration"
//@ diag: 26:3 error "'const' is invalid in friend declarations"
//@ diag: 27:3 error "'static' is invalid in friend declarations"
//@ diag: 27:10 error "'friend' must appear first in a non-function declaration"
//@ diag: 28:3 error "'virtual' is invalid in friend declarations"
//@ diag: 28:11 error "'friend' must appear first in a non-function declaration"
//@ diag: 29:3 error "class cannot be marked constexpr"
//@ diag: 30:10 error "must use a qualified name when declaring a conversion operator as a friend"
//@ diag: 31:10 error "must use a qualified name when declaring a destructor as a friend"
//@ diag: 32:25 error "non-member function cannot have 'const' qualifier"
//@ diag: 32:10 error "must use a qualified name when declaring a conversion operator as a friend"
class c;
struct B { ~B(); operator int(); };
struct A {
  inline friend class c;
  const friend class c;
  static friend class c;
  virtual friend class c;
  constexpr friend class c;
  friend operator unsigned char();
  friend ~B();
  friend operator int() const;
};
