// [class.access]/1, [class.access.spec]: private members are usable only by
// members and friends of their class; the default access of a class is
// private, of a struct public.
//@ expect: compile-fail
//@ strict-diags
//@ xfail(kiln): needs WP-065 parser classes
//@ diag: 30:5 error "'f' is a private member of 'B'"
//@ diag: 27:8 note "declared private here"
//@ diag: 31:12 error "'x' is a private member of 'A'"
//@ diag: 16:7 note "implicitly declared private here"
//@ diag: 31:35 error "'z' is a protected member of 'A'"
//@ diag: 21:14 note "declared protected here"
//@ diag: 31:48 error "'e1' is a protected member of 'A'"
//@ diag: 22:10 note "declared protected here"
class A {
  int x;
public:
  int y;
  int get() const { return x; }
protected:
  static int z;
  enum { e1 };
};
struct B {
  int w;
private:
  void f();
};
int use(A a, B b) {
  b.f();
  return a.x + a.y + a.get() + A::z + b.w + A::e1;
}
