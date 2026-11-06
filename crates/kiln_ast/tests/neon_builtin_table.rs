//! The `AArch64` NEON / FP16 target builtin table
//! (spec/landed/sema/WP-558.md "Target builtins"), in its plain-data
//! layout (spec/landed/sema/WP-653.md): every row decodes, ids and name
//! lookups agree, and the decoded rows are byte-identical to the literal
//! rows the table held before WP-653. Implementation tables, no standard
//! section.

use kiln_ast::decl::builtin::{self, attr, neon};

/// FNV-1a 64 of every row's `name NUL prototype NUL features NUL`, in row
/// order, over the pre-WP-653 literal table (1,081 `r(index, name, sig,
/// features)` rows): the decoded table must hash the same.
const PRE_WP653_HASH: u64 = 0xfe9d_d2d8_389f_8206;

/// Sample rows of the pre-WP-653 literal table, verbatim:
/// `r(index, name, prototype, features)`.
const SAMPLES: &[(u32, &str, &str, &str)] = &[
    (0, "__builtin_neon___a32_vcvt_bf16_f32", "V8ScV16Sci", "bf16,neon"),
    (1, "__builtin_neon___a64_vcvtq_low_bf16_f32", "V16ScV16Sci", "bf16,neon"),
    (13, "__builtin_neon_vabdh_f16", "hhh", "fullfp16"),
    (35, "__builtin_neon_vaddlvq_s32", "WiV4i", "neon"),
    (60, "__builtin_neon_vaeseq_u8", "V16ScV16ScV16Sci", "aes,neon"),
    (188, "__builtin_neon_vcmla_f64", "V8ScV8ScV8ScV8Sci", "v8.3a,neon"),
    (207, "__builtin_neon_vcmlaq_rot90_f16", "V16ScV16ScV16ScV16Sci", "v8.3a,fullfp16,neon"),
    (391, "__builtin_neon_vdotq_s32", "V16ScV16ScV16ScV16Sci", "dotprod,neon"),
    (440, "__builtin_neon_vfmlalq_high_f16", "V16ScV16ScV16ScV16Sci", "fp16fml,neon"),
    (485, "__builtin_neon_vld1q_v", "V16ScvC*i", "neon"),
    (531, "__builtin_neon_vldap1q_lane_s64", "V16ScvC*V16ScIii", "rcpc3,neon"),
    (598, "__builtin_neon_vmmlaq_s32", "V16ScV16ScV16ScV16Sci", "i8mm,neon"),
    (1078, "__builtin_neon_vxarq_u64", "V16ScV16ScV16ScIii", "sha3,neon"),
    (1080, "__builtin_neon_vzipq_v", "vv*V16ScV16Sci", "neon"),
];

#[test]
fn every_row_decodes_and_round_trips() {
    let rows: Vec<neon::Row> = neon::rows().collect();
    assert_eq!(rows.len(), neon::len());
    assert_eq!(rows.len(), 1081);
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for (i, r) in rows.iter().enumerate() {
        let id = neon::BASE + u32::try_from(i).unwrap();
        assert_eq!(r.info.id, id);
        assert_eq!(r.info.attrs, attr::NOTHROW, "{}", r.info.name);
        assert!(neon::is_neon(id));
        assert_eq!(neon::by_name(r.info.name), Some(id), "{}", r.info.name);
        assert_eq!(builtin::by_name(r.info.name), Some(id), "{}", r.info.name);
        let info = builtin::info(id).unwrap();
        assert_eq!((info.name, info.sig, info.attrs), (r.info.name, r.info.sig, r.info.attrs));
        assert_eq!(neon::required_features(r.info.name), Some(r.features));
        assert!(r.info.name.starts_with(neon::PREFIX), "{}", r.info.name);
        assert!(!r.info.sig.is_empty() && !r.features.is_empty(), "{}", r.info.name);
        for s in [r.info.name, r.info.sig, r.features] {
            for b in s.bytes().chain([0]) {
                hash = (hash ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
            }
        }
    }
    assert!(rows.windows(2).all(|w| w[0].info.name < w[1].info.name), "sorted, distinct");
    assert_eq!(hash, PRE_WP653_HASH, "the decoded table differs from the pre-WP-653 rows");
}

#[test]
fn sample_rows_carry_their_fields() {
    for &(index, name, sig, features) in SAMPLES {
        let id = neon::BASE + index;
        let r = neon::row(id).unwrap();
        assert_eq!((r.info.id, r.info.name, r.info.sig, r.features), (id, name, sig, features));
        assert_eq!(r.info.attrs, attr::NOTHROW);
        assert_eq!(neon::by_name(name), Some(id));
    }
    assert_eq!(neon::required_features("__builtin_neon_vdotq_s32"), Some("dotprod,neon"));
    assert!(neon::features_hold("dotprod,neon", |f| matches!(f, "dotprod" | "neon")));
    assert_eq!(neon::first_missing("dotprod,neon", |f| f == "neon"), Some("dotprod"));
}

#[test]
fn ids_outside_the_table() {
    let end = neon::BASE + u32::try_from(neon::len()).unwrap();
    for id in [0, neon::BASE - 1, end, u32::MAX] {
        assert!(!neon::is_neon(id) && neon::row(id).is_none(), "{id:#x}");
    }
    assert!(neon::row(end - 1).is_some());
    assert_eq!(neon::by_name("__builtin_neon_not_a_builtin"), None);
    assert_eq!(neon::by_name(neon::PREFIX), None);
    assert_eq!(neon::by_name(""), None);
    assert_eq!(neon::required_features("__builtin_neon_not_a_builtin"), None);
}
