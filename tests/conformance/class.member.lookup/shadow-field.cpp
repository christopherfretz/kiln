// [class.member.lookup], [class.access.base]: -Wshadow-field (in
// -Wshadow-all) diagnoses a non-static data member or a member function
// parameter that hides a non-private member of a base class reachable
// through an accessible path, once per base; a virtual base counts once.
//@ verify
//@ args: -Wshadow-field

struct A { int a; int pa; private: int pr; }; // expected-note {{declared here}} expected-note {{declared here}} expected-note {{declared here}} expected-note {{declared here}}
struct PB : private A {};
struct V1 : virtual A {};
struct V2 : virtual A {};
struct D1 : V1, V2 { int a; void m(int pa); }; // expected-warning {{non-static data member 'a' of 'D1' shadows member inherited from type 'A'}}
void D1::m(int pa) { (void)pa; } // expected-warning {{parameter 'pa' shadows member inherited from type 'A'}}
struct E : PB { int a; };
class G : A { int pa; int pr; }; // expected-warning {{non-static data member 'pa' of 'G' shadows member inherited from type 'A'}}
struct U { union { int ua; }; }; // expected-note {{declared here}} expected-note {{declared here}}
struct H : U { int ua; void n(int ua) { (void)ua; } }; // expected-warning {{non-static data member 'ua' of 'H' shadows member inherited from type 'U'}} expected-warning {{parameter 'ua' shadows member inherited from type 'U'}}
template <class T> struct TB { int t; }; // expected-note {{declared here}} expected-note {{declared here}}
template <class T> struct TD : TB<T> { int t; };
template <class T> struct TE : TB<int> { int t; void q(int t); }; // expected-warning {{non-static data member 't' of 'TE' shadows member inherited from type 'TB<int>'}}
template <class T> void TE<T>::q(int t) { (void)t; } // expected-warning {{parameter 't' shadows member inherited from type 'TB<int>'}}
TD<int> tdi; TE<int> tei;
struct W : A { int a; static int pa; void s(int a); }; // expected-warning {{non-static data member 'a' of 'W' shadows member inherited from type 'A'}}
