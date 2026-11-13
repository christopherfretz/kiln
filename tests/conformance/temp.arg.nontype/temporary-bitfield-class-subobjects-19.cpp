// [temp.arg.nontype]/6: a pointer or reference subobject of a class-type
// template argument must not designate a temporary object or a subobject
// of one; a pointer of another type than the temporary (`&rp.a`, at
// offset 0) designates a subobject (clang names the designator path).
// [temp.arg.nontype]/2, [expr.const]/12, [dcl.init.ref]/5: the argument
// of a reference parameter is a converted constant expression, in which
// a reference never binds to a bit-field (clang
// `err_reference_bind_to_bitfield_in_cce`; clang 19 has no parameter
// note, this is the clang 19 form).
//@ expect: compile-fail
//@ requires: clang<22
//@ strict-diags
// The clang 22 sibling is temporary-bitfield-class-subobjects.cpp, which
// also pins the "template parameter is declared here" notes.
//@ diag: 24:3 error "pointer to subobject of temporary object is not allowed in a template argument"
//@ diag: 25:3 error "pointer to subobject of temporary object is not allowed in a template argument"
//@ diag: 27:3 error "reference to temporary object is not allowed in a template argument"
//@ diag: 31:3 error "reference cannot bind to bit-field in converted constant expression"
//@ diag: 32:3 error "reference cannot bind to bit-field in converted constant expression"
template <auto V> struct X {};
struct P { int a, b; };
constexpr const P& rp = P{1, 2};
struct Q { const int* p; };
X<Q{&rp.b}> q1;
X<Q{&rp.a}> q2;
struct QR { const P& r; };
X<QR{rp}> q3;
struct B { int b : 3; }; B bb{1};
template <const int& r> struct Y {};
template <int& r> struct Z {};
Y<(bb.b)> y;
Z<bb.b> z;
