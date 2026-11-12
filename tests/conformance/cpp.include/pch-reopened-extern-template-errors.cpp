// [temp.explicit]/2, [temp.spec.general]/5: an explicit instantiation
// definition of a specialization the header already explicitly
// instantiated is an error, also through the precompiled image
// (WP-1320).
//@ expect: compile-fail
//@ diag: 8:22 error "duplicate explicit instantiation of 'basic<long>'"
#include "Inputs/reopened_extern_template.h"
template struct ret::basic<long>;
