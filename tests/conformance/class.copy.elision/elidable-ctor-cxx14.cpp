// [class.copy.elision]/1.3 (C++14): copy-initializing an object from a
// prvalue of its class copies or moves a temporary, a copy that may be
// elided but whose constructor must be usable. Found running GCC's C++
// testsuite.
//@ std: c++14
//@ expect: compile-fail
//@ strict-diags
//@ diag: 23:13 error "call to deleted constructor of 'Im'"
//@ diag: 20:13 note "'Im' has been explicitly marked deleted here"
//@ diag: 23:30 error "call to deleted constructor of 'Im'"
//@ diag: 20:13 note "'Im' has been explicitly marked deleted here"
//@ diag: 24:13 error "call to deleted constructor of 'Im'"
//@ diag: 20:13 note "'Im' has been explicitly marked deleted here"
//@ diag: 24:34 error "call to deleted constructor of 'Im'"
//@ diag: 20:13 note "'Im' has been explicitly marked deleted here"
//@ diag: 27:18 error "call to deleted constructor of 'Nc'"
//@ diag: 25:19 note "'Nc' has been explicitly marked deleted here"
//@ diag: 30:17 error "calling a private constructor of class 'Pv'"
//@ diag: 28:28 note "declared private here"
struct Im { Im(Im&&) = delete; };
struct H { static Im mk(); };
Im mk();
Im a() { Im m = mk(); return mk(); }
Im b() { Im m = H().mk(); return H().mk(); }
struct Nc { Nc(); Nc(const Nc&) = delete; };
Nc nf();
struct W { W() : n(nf()) {} Nc n; };
struct Pv { Pv(); private: Pv(Pv&&); };
Pv pf();
Pv c() { return pf(); }
