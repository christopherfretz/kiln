//@ verify
// [namespace.udecl]/19: a conversion function named by a using-declaration
// has the using-declaration's access.
struct B { operator int() const { return 1; } };
struct I : B { private: using B::operator int; }; // expected-note {{declared private here}}
struct J : private B { protected: using B::operator int; }; // expected-note {{declared protected here}}
struct K : J { int get() const { return *this; } };
int main() {
  I i;
  int a = i; // expected-error {{'operator int' is a private member of 'I'}}
  J j;
  int b = j; // expected-error {{'operator int' is a protected member of 'J'}}
  return a + b + K().get();
}
