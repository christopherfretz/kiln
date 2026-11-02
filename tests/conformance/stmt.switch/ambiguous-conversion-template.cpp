// [conv.general]/5, [over.match.conv]: an ambiguous contextual implicit
// conversion notes every non-explicit conversion function, a conversion
// function template included: by its specialization deduced against the
// unique target type ("candidate function [with T = int]"), or as
// "candidate function template" when the targets differ and nothing was
// deduced. clang 19-22 crash on these (diagnoseAmbiguousConversion casts
// the template to a conversion function), so the wording is kiln's.
//@ expect: compile-fail
//@ std: c++26
//@ strict-diags
//@ skip(clang): clang crashes noting a conversion function template (diagnoseAmbiguousConversion)
//@ diag: 31:3 error "multiple conversions from switch condition type 'Same' to an integral or enumeration type"
//@ diag: 24:34 note "candidate function [with T = int]"
//@ diag: 24:63 note "conversion to integral type 'int'"
//@ diag: 24:85 note "conversion to integral type 'int'"
//@ diag: 32:3 error "multiple conversions from switch condition type 'Two' to an integral or enumeration type"
//@ diag: 25:33 note "candidate function template"
//@ diag: 25:53 note "conversion to integral type 'int'"
//@ diag: 25:75 note "conversion to integral type 'long'"
//@ diag: 33:3 error "ambiguous conversion of delete expression of type 'Ptrs' to a pointer"
//@ diag: 26:34 note "candidate function template"
//@ diag: 26:55 note "conversion to pointer type 'int *'"
//@ diag: 26:78 note "conversion to pointer type 'char *'"
struct Same { template <class T> operator T() const volatile; operator int() const; operator int() volatile; };
struct Two { template <class T> operator T() const; operator int() const; operator long() const; };
struct Ptrs { template <class T> operator T*() const; operator int*() const; operator char*() const; };
struct One { template <class T> operator T() const; operator int() const; };
struct OnePtr { template <class T> operator T*() const; operator int*() const; };

void f(Same s, Two t, Ptrs p, One o, OnePtr op) {
  switch (s) { case 0: break; }
  switch (t) { case 0: break; }
  delete p;
  switch (o) { case 0: break; }
  delete op;
}
