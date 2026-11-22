// [class.qual]/2: after a non-dependent nested-name-specifier naming the
// class C, `C::C<args>` and `C::C` name C's constructors, not the template
// or the type: clang's error "qualified reference to 'C' is a constructor
// name rather than a template name" (or "a type") in a declaration, an
// expression or an alias, and with a preceding `typename` or `template`
// keyword its -Winjected-class-name warning naming the keyword (the
// template or type is used). A class-key, a base-specifier or a
// mem-initializer names the class, and so does a name followed by `::`.
// WP-1903.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 35:36 warning "ISO C++ specifies that qualified reference to 'mapping' is a constructor name rather than a template name in this context, despite preceding 'template' keyword"
//@ diag: 36:27 warning "ISO C++ specifies that qualified reference to 'mapping' is a constructor name rather than a template name in this context, despite preceding 'typename' keyword"
//@ diag: 37:13 warning "ISO C++ specifies that qualified reference to 'S' is a constructor name rather than a type in this context, despite preceding 'typename' keyword"
//@ diag: 38:27 warning "ISO C++ specifies that qualified reference to 'mapping' is a constructor name rather than a template name in this context, despite preceding 'template' keyword"
//@ diag: 39:18 error "qualified reference to 'mapping' is a constructor name rather than a template name in this context"
//@ diag: 40:4 error "qualified reference to 'mapping' is a constructor name rather than a template name in this context"
//@ diag: 41:4 error "qualified reference to 'S' is a constructor name rather than a type in this context"
//@ diag: 42:18 error "qualified reference to 'mapping' is a constructor name rather than a template name in this context"
//@ diag: 43:25 error "qualified reference to 'mapping' is a constructor name rather than a template name in this context"
//@ diag: 44:29 error "qualified reference to 'mapping' is a constructor name rather than a template name in this context"
//@ diag: 45:38 warning "ISO C++ specifies that qualified reference to 'mapping' is a constructor name rather than a template name in this context, despite preceding 'typename' keyword"
//@ diag: 47:30 error "qualified reference to 'mapping' is a constructor name rather than a template name in this context"
//@ diag: 48:38 warning "ISO C++ specifies that qualified reference to 'mapping' is a constructor name rather than a template name in this context, despite preceding 'template' keyword"
//@ diag: 50:42 error "qualified reference to 'mapping' is a constructor name rather than a template name in this context"
struct L {
  template <class T> struct mapping {
    template <class U> mapping(U);
    mapping();
    using type = int;
  };
};
struct S { int x; };
using M = L::mapping<int>;
typename L::mapping<int>::template mapping<long> *p1;
typename L::mapping<int>::mapping<long> *p2;
typename S::S *p3;
L::mapping<int>::template mapping<long> *p4;
L::mapping<int>::mapping<long> *p5;
M::mapping<long> *p6;
S::S *p7;
L::mapping<int>::mapping<long> f();
void g(L::mapping<int>::mapping<long>);
using A1 = L::mapping<int>::mapping<long>;
using A2 = typename L::mapping<int>::mapping<long>;
template <class T> struct Q {
  using t = L::mapping<int>::mapping<T>;
  typename L::mapping<int>::template mapping<long> *q;
};
void h() { (void)sizeof(L::mapping<int>::mapping<long>); }
class L::mapping<int>::template mapping<long> *c1;
struct S::S *c2;
L::mapping<int>::mapping<long>::type c3;
typename L::mapping<int>::mapping<long>::type c4;
struct Z1 : L::mapping<int>::mapping<long> {
  Z1() : L::mapping<int>::mapping<long>() {}
};
struct Z2 : S::S {
  Z2() : S::S() {}
};
template <class T> struct Y : L::mapping<int>::template mapping<T> {
  Y() : L::mapping<int>::template mapping<T>() {}
};
Y<int> y;
