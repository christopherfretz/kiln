// typeid of an incomplete class type ([expr.typeid]/4): references and
// cv-qualifiers are looked through; pointers, void and incomplete arrays
// of non-class type are fine.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 18:9 error "'typeid' of incomplete type 'I'"
//@ diag: 15:8 note "forward declaration of 'I'"
//@ diag: 19:9 error "'typeid' of incomplete type 'I'"
//@ diag: 15:8 note "forward declaration of 'I'"
//@ diag: 20:9 error "'typeid' of incomplete type 'I'"
//@ diag: 15:8 note "forward declaration of 'I'"
//@ diag: 21:9 error "'typeid' of incomplete type 'I'"
//@ diag: 15:8 note "forward declaration of 'I'"
namespace std { class type_info; }
struct I;
I &get();
void f(I *p) {
  (void)typeid(I);
  (void)typeid(const I &);
  (void)typeid(get());
  (void)typeid(*p);
  (void)typeid(I *);
  (void)typeid(void);
  (void)typeid(int[]);
  (void)typeid(p);
}
