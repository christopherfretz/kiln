//@ expect: ast
//@ ast-filter: f
// [dcl.init.general]/16.6.1-2, [class.copy.ctor]/1, /3, [over.ics.ref],
// [class.temporary]/2: initializing an object from an object of the same
// or a derived class calls the copy (lvalue) or move (xvalue) constructor;
// a derived source is converted to the base through a const lvalue (a
// derived prvalue is materialized first); a prvalue of the same class
// initializes the object directly. The implicit constructors' exception
// specifications are resolved when used ([except.spec]/7). By-value
// arguments are bound temporaries ([expr.call]/7); explicit conversions
// are ConstructorConversion casts ([expr.static.cast]/4,
// [expr.type.conv]/1). Matches clang 19.
namespace q {
struct S { int a; ~S(); };
struct D : S {};
struct T { int a; };
S mk(); D mkd(); S &&xv();
void take(S); void takec(const S);
void f(S a, D d, T t, const S cs) {
  S b = a;
  S c(a);
  S e = xv();
  S g = d;
  S h = mkd();
  S i = mk();
  T j = t;
  const S k = a;
  take(a);
  takec(a);
  take(d);
  (void)S(a);
  (void)static_cast<S>(d);
  (void)(S)cs;
  (void)T(t);
  (void)S(mk());
  (void)static_cast<const S>(a);
}
}
