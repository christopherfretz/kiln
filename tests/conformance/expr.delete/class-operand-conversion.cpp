// [expr.delete]/2, [conv.general]/5: an operand of class type is
// contextually implicitly converted to a pointer to object type by its
// non-explicit conversion functions (C++14 rules, clang
// PerformContextualImplicitConversion; the helper shared with `switch`).
//@ expect: compile-fail
//@ std: c++26
//@ strict-diags
//@ diag: 35:3 error "ambiguous conversion of delete expression of type 'Two' to a pointer"
//@ diag: 23:14 note "conversion to pointer type 'int *'"
//@ diag: 23:37 note "conversion to pointer type 'char *'"
//@ diag: 36:3 error "ambiguous conversion of delete expression of type 'Same' to a pointer"
//@ diag: 24:15 note "conversion to pointer type 'int *'"
//@ diag: 24:38 note "conversion to pointer type 'int *'"
//@ diag: 37:3 error "converting delete expression from type 'Ex' to type 'int *' invokes an explicit conversion function"
//@ diag: 25:22 note "conversion to pointer type 'int *'"
//@ diag: 38:3 error "deleting incomplete class type 'Inc'; no conversions to pointer type"
//@ diag: 26:8 note "forward declaration of 'Inc'"
//@ diag: 39:3 error "cannot delete expression of type 'None'"
//@ diag: 40:3 error "cannot delete expression of type 'Fn'"
//@ diag: 41:3 error "cannot delete expression of type 'Del'"
//@ diag: 43:3 error "cannot delete expression of type 'TplOnly'"
//@ diag: 44:3 error "cannot delete pointer to incomplete type 'void'"
struct Two { operator int*() const; operator char*() const; };
struct Same { operator int*() const; operator int*() volatile; };
struct Ex { explicit operator int*() const; };
struct Inc;
struct None { };
struct Fn { using fp = void (*)(); operator fp() const; };
struct Del { operator int*() const = delete; };
struct Tpl { template <class T> operator T*() const; operator int*() const; };
struct TplOnly { template <class T> operator T*() const; operator int() const; };
struct Vp { operator void*() const; };

void f(Two two, Same same, Ex ex, Inc& inc, None n, Fn fn, Del d, Tpl t, TplOnly to, Vp vp) {
  delete two;
  delete same;
  delete ex;
  delete inc;
  delete n;
  delete fn;
  delete d;
  delete t;
  delete to;
  delete vp;
}
