// [over.match.ctor]/1, [class.copy.ctor]/8, [temp.inst]/3: the implicit
// copy and move constructors of an implicitly instantiated class template
// specialization are noted at the definition it was instantiated from
// (the primary template's or the partial specialization's), not at an
// earlier declaration of the template; an explicit instantiation or
// explicit specialization keeps its own name.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 31:8 error "no matching constructor for initialization of 'B<int>'"
//@ diag: 30:12 note "candidate constructor not viable: requires 1 argument, but 2 were provided"
//@ diag: 30:8 note "candidate constructor (the implicit copy constructor) not viable: requires 1 argument, but 2 were provided"
//@ diag: 30:8 note "candidate constructor (the implicit move constructor) not viable: requires 1 argument, but 2 were provided"
//@ diag: 35:9 error "no matching constructor for initialization of 'C<int *>'"
//@ diag: 34:35 note "candidate constructor not viable: requires 1 argument, but 2 were provided"
//@ diag: 34:27 note "candidate constructor (the implicit copy constructor) not viable: requires 1 argument, but 2 were provided"
//@ diag: 34:27 note "candidate constructor (the implicit move constructor) not viable: requires 1 argument, but 2 were provided"
//@ diag: 38:9 error "no matching constructor for initialization of 'B<long>'"
//@ diag: 30:12 note "candidate constructor not viable: requires 1 argument, but 2 were provided"
//@ diag: 37:17 note "candidate constructor (the implicit copy constructor) not viable: requires 1 argument, but 2 were provided"
//@ diag: 37:17 note "candidate constructor (the implicit move constructor) not viable: requires 1 argument, but 2 were provided"
//@ diag: 41:8 error "no matching constructor for initialization of 'D<int>'"
//@ diag: 40:29 note "candidate constructor not viable: requires 1 argument, but 2 were provided"
//@ diag: 40:20 note "candidate constructor (the implicit copy constructor) not viable: requires 1 argument, but 2 were provided"
//@ diag: 40:20 note "candidate constructor (the implicit move constructor) not viable: requires 1 argument, but 2 were provided"

template <class T> struct B;
B<int> *p;
template <class T> struct C;
template <class T>
struct B { B(int); };
B<int> b(1, 2);
template <class T> struct C;
B<char> *q;
template <class T> struct C<T*> { C(int); };
C<int*> c(1, 2);
template <class T> struct D;
template struct B<long>;
B<long> bl(1, 2);
template <class T> struct D;
template <> struct D<int> { D(int); };
D<int> d(1, 2);
