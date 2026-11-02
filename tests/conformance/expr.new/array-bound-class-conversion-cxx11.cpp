// C++11 [expr.new]/6: the first array bound is contextually implicitly
// converted to an integral or unscoped enumeration type (clang
// PerformContextualImplicitConversion; the helper shared with `switch`),
// diagnosed at the new-expression.
//@ expect: compile-fail
//@ std: c++11
//@ strict-diags
//@ diag: 31:3 error "ambiguous conversion of array size expression of type 'Two' to an integral or enumeration type"
//@ diag: 22:14 note "conversion to integral type 'int' declared here"
//@ diag: 22:36 note "conversion to integral type 'long' declared here"
//@ diag: 32:3 error "array size expression of type 'Ex' requires explicit conversion to type 'int'"
//@ diag: 23:22 note "conversion to integral type 'int' declared here"
//@ diag: 33:3 error "array size expression must have integral or unscoped enumeration type, not 'None'"
//@ diag: 34:3 error "array size expression has incomplete class type 'Inc'"
//@ diag: 25:8 note "forward declaration of 'Inc'"
//@ diag: 35:3 error "array size expression must have integral or unscoped enumeration type, not 'Scoped'"
//@ diag: 38:3 error "array size expression must have integral or unscoped enumeration type, not 'int *'"
//@ diag: 39:3 error "array size expression must have integral or unscoped enumeration type, not 'double'"
//@ diag: 40:3 error "array size expression must have integral or unscoped enumeration type, not 'Sc'"
enum E { e0 = 2 };
enum class Sc { a };
struct Two { operator int() const; operator long() const; };
struct Ex { explicit operator int() const; };
struct None { };
struct Inc;
struct Scoped { operator Sc() const; };
struct Ok { operator int() const; };
struct En { operator E() const; };

void f(Two two, Ex ex, None n, Inc& inc, Scoped s, Ok ok, En en, int* p, double d, Sc sc) {
  new int[two];
  new int[ex];
  new int[n];
  ::new int[inc];
  new int[s];
  new int[ok];
  new int[en];
  new int[p];
  new int[d];
  new int[sc];
}
