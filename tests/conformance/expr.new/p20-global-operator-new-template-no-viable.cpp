// [expr.new]/20, [over.match.viable]: a global allocation function
// template takes part in overload resolution; when nothing is viable its
// deduced specialization is listed among the candidates (WP-712).
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:3 error "no matching function for call to 'operator new'"
//@ diag: 10:26 note "candidate function template not viable: no known conversion from 'S' to 'int' for 3rd argument"
typedef __SIZE_TYPE__ size_t;
template <class A> void* operator new(size_t, A*, int);
struct S {};
int main() {
  int x;
  new (&x, S()) int(1);
}
