// [class.friend]/3, [dcl.type.elab]/4: an elaborated enum specifier cannot be
// declared as a friend (clang -Wfriend-enum warning with a fix-it note);
// 'friend E;' is the valid form, and a dependent 'friend typename T::G;'
// is fine. WP-1017.
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 26:10 warning "elaborated enum specifier cannot be declared as a friend"
//@ diag: 26:10 note "remove 'enum' to befriend an enum"
//@ diag: 27:10 warning "elaborated enum specifier cannot be declared as a friend"
//@ diag: 27:10 note "remove 'enum' to befriend an enum"
//@ diag: 28:15 error "reference to enumeration must use 'enum' not 'enum class'"
//@ diag: 28:10 warning "elaborated enum specifier cannot be declared as a friend"
//@ diag: 28:10 note "remove 'enum class' to befriend an enum"
//@ diag: 29:10 warning "elaborated enum specifier cannot be declared as a friend"
//@ diag: 29:10 note "remove 'enum' to befriend an enum"
//@ diag: 35:10 warning "elaborated enum specifier cannot be declared as a friend"
//@ diag: 35:10 note "remove 'enum' to befriend an enum"
//@ diag: 36:10 warning "elaborated enum specifier cannot be declared as a friend"
//@ diag: 36:10 note "remove 'enum' to befriend an enum"
enum class E;
enum F : int;
template <typename T> struct A { enum class G; };
struct B {
  enum class H;
  friend enum E;
  friend enum F;
  friend enum class E;
  friend enum B::H;
  friend E;
  friend F;
  friend H;
};
template <typename T> struct C {
  friend enum T::G;
  friend enum A<T>::G;
  friend typename T::G;
};
C<A<int>> c;
