// [WP-3698] Constant evaluation and IR emission of `ext_vector_type` bool
// splats (`CastKind::BooleanToSignedIntegral`): `true` splats to -1 / all-ones
// (-1.0f for floating vectors), `false` to 0, and a dynamic `bool` global
// initializes dynamically without panicking in the Oven.
typedef int V4i __attribute__((ext_vector_type(4)));
typedef signed char V4sc __attribute__((ext_vector_type(4)));
typedef char V4c __attribute__((ext_vector_type(4)));
typedef long V2l __attribute__((ext_vector_type(2)));
typedef unsigned int V4u __attribute__((ext_vector_type(4)));
typedef unsigned char V4uc __attribute__((ext_vector_type(4)));
typedef unsigned long V2ul __attribute__((ext_vector_type(2)));
typedef float V4f __attribute__((ext_vector_type(4)));
typedef __int128 V2i128 __attribute__((ext_vector_type(2)));

constexpr V4i cv_it = (V4i)true;
constexpr V4i cv_if = (V4i)false;
constexpr V4sc cv_sct = (V4sc)true;
constexpr V4sc cv_scf = (V4sc)false;
constexpr V4c cv_ct = (V4c)true;
constexpr V2l cv_lt = (V2l)true;
constexpr V2l cv_lf = (V2l)false;
constexpr V4u cv_ut = (V4u)true;
constexpr V4u cv_uf = (V4u)false;
constexpr V4uc cv_uct = (V4uc)true;
constexpr V2ul cv_ult = (V2ul)true;
constexpr V4f cv_ft = (V4f)true;
constexpr V4f cv_ff = (V4f)false;
constexpr V2i128 cv_i128t = (V2i128)true;
constexpr V2i128 cv_i128f = (V2i128)false;

bool bb;
V4i dyn_v = (V4i)bb;

const void *keep[] = {
    &cv_it,  &cv_if,  &cv_sct, &cv_scf, &cv_ct,  &cv_lt,   &cv_lf,   &cv_ut,
    &cv_uf,  &cv_uct, &cv_ult, &cv_ft,  &cv_ff,  &cv_i128t, &cv_i128f, &dyn_v,
};

V4i dyn_fn(bool b) { return (V4i)b; }
