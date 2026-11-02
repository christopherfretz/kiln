// [stmt.switch]/2, [conv.general]/5: a condition of class type is
// contextually implicitly converted to the unique integral or enumeration
// type its non-explicit conversion functions yield (C++14 rules, clang
// PerformContextualImplicitConversion).
//@ expect: compile-fail
//@ std: c++26
//@ strict-diags
//@ diag: 41:3 error "switch condition type 'Ex' requires explicit conversion to 'int'"
//@ diag: 25:22 note "conversion to integral type 'int'"
//@ diag: 42:3 error "multiple conversions from switch condition type 'Two' to an integral or enumeration type"
//@ diag: 26:14 note "conversion to integral type 'int'"
//@ diag: 26:36 note "conversion to integral type 'long'"
//@ diag: 43:3 error "multiple conversions from switch condition type 'Mixed' to an integral or enumeration type"
//@ diag: 28:16 note "conversion to integral type 'int'"
//@ diag: 28:38 note "conversion to enumeration type 'E'"
//@ diag: 44:3 error "statement requires expression of integer type ('None' invalid)"
//@ diag: 45:3 error "statement requires expression of integer type ('Del' invalid)"
//@ diag: 46:3 error "statement requires expression of integer type ('const NC' invalid)"
//@ diag: 47:3 error "statement requires expression of integer type ('Tpl' invalid)"
//@ diag: 48:3 error "switch condition has incomplete class type 'Inc'"
//@ diag: 36:8 note "forward declaration of 'Inc'"
//@ diag: 49:3 warning "switch condition has boolean value"
//@ diag: 50:11 warning "enumeration value 'B' not handled in switch"
enum E { A, B };
struct Ex { explicit operator int() const; };
struct Two { operator int() const; operator long() const; };
struct Ok2 { operator int() &; operator int() &&; };
struct Mixed { operator int() const; operator E() const; };
struct None { operator double() const; };
struct Del { operator int() const = delete; };
struct NC { operator int(); };
struct Tpl { template <class T> operator T() const; };
struct Bool { operator bool() const; };
struct En { operator E() const; };
struct Ref { operator const int&() const; };
struct Inc;
enum class Sc { X, Y };
struct Scoped { operator Sc() const; };

void f(Ex ex, Two two, Ok2 ok, Mixed mx, None n, Del d, const NC& nc, Tpl t, Inc& inc, Bool b, En en, Ref r, Scoped s) {
  switch (ex) { case 1: break; }
  switch (two) { case 1: break; }
  switch (mx) { case 1: break; }
  switch (n) { case 1: break; }
  switch (d) { case 1: break; }
  switch (nc) { case 1: break; }
  switch (t) { case 1: break; }
  switch (inc) { case 1: break; }
  switch (b) { case 1: break; }
  switch (en) { case A: break; }
  switch (ok) { case 1: break; }
  switch (Ok2{}) { case 1: break; }
  switch (r) { case 1: break; }
  switch (s) { case Sc::X: case Sc::Y: break; }
}
