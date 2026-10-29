//@ expect: compile-fail
//@ strict-diags
//@ diag: 6:42 error "concept cannot have associated constraints"
//@ diag: 7:33 error "ISO C++ does not permit the 'bool' keyword after 'concept'"
// [temp.concept]/4: a concept shall not have associated constraints
template <class T> requires true concept C = true;
template <class T> concept bool D = true;
