//! The x86-64 `-march=` CPU and target-feature tables
//! (spec/landed/driver/WP-630.md "Tables"), in their plain-data layout
//! (spec/landed/driver/WP-660.md): every row decodes, lookups find their own
//! rows, sample rows carry the lists the pre-WP-660 literal tables held, and
//! both tables hash to the pre-WP-660 tables' hashes. Implementation tables,
//! no standard section.

use kiln_driver::x86_targets as t;

/// FNV-1a (64-bit) of `bytes`, continuing from `h`.
fn fnv(mut h: u64, bytes: &[u8]) -> u64 {
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

fn join(l: t::List) -> String {
    l.iter().collect::<Vec<_>>().join(",")
}

fn sorted<T: Ord>(v: &[T]) -> bool {
    v.windows(2).all(|w| w[0] < w[1])
}

#[test]
fn every_cpu_decodes() {
    let cpus: Vec<t::Cpu> = t::cpus().collect();
    assert_eq!(cpus.len(), 72, "spec/landed/driver/WP-630.md counts 72 CPUs");
    for c in &cpus {
        let found = t::cpu(c.name).unwrap();
        assert_eq!((found.name, join(found.features)), (c.name, join(c.features)));
        let features: Vec<&str> = c.features.iter().collect();
        assert!(sorted(&features), "{}", c.name);
        assert_eq!(features.len(), c.features.len());
        let define: Vec<(&str, &str)> = c.define.iter().collect();
        assert!(sorted(&define), "{}", c.name);
        assert_eq!(define.len(), c.define.len());
        let undef: Vec<&str> = c.undef.iter().collect();
        assert!(sorted(&undef), "{}", c.name);
        for f in features {
            assert!(t::implies(f).is_some(), "{}: {f}", c.name);
        }
    }
    let names: Vec<&str> = cpus.iter().map(|c| c.name).collect();
    assert_eq!(names[..3], ["nocona", "core2", "penryn"]);
    assert_eq!(names[69..], ["x86-64-v3", "x86-64-v4", "diamondrapids"]);
    for n in ["", "native", "wildcatlake", "novalake", "Skylake"] {
        assert!(t::cpu(n).is_none(), "{n}");
    }
    let c = t::cpu("x86-64").unwrap();
    assert_eq!(join(c.features), "cmov,cx8,fxsr,mmx,sse,sse2,x87");
    assert!(c.define.is_empty() && c.undef.is_empty());
}

/// The hash of every CPU as `name\0features\0macro=value,...\0undef\n`
/// (`,`-joined lists) and of every implication as `feature\0features\n`,
/// computed from main's literal `Cpu { name: "...", ... }` and `("f", &[...])`
/// rows at d47d9d1a6 (before WP-660): the decoded tables are byte-identical
/// to them.
#[test]
fn whole_tables_match_the_literal_ones() {
    let mut h = 0xcbf2_9ce4_8422_2325;
    for c in t::cpus() {
        let define: Vec<String> = c.define.iter().map(|(n, v)| format!("{n}={v}")).collect();
        let line =
            format!("{}\0{}\0{}\0{}\n", c.name, join(c.features), define.join(","), join(c.undef));
        h = fnv(h, line.as_bytes());
    }
    assert_eq!(h, 0xad0d_c6e1_2358_8b7c);
    let mut h = 0xcbf2_9ce4_8422_2325;
    for (f, l) in t::implications() {
        h = fnv(h, format!("{f}\0{}\n", join(l)).as_bytes());
    }
    assert_eq!(h, 0x995f_b461_3471_dbae);
}

#[test]
fn cpus_carry_their_lists() {
    let c = t::cpu("nocona").unwrap();
    assert_eq!(join(c.features), "cmov,cx16,cx8,fxsr,mmx,sse,sse2,sse3,x87");
    assert_eq!(
        c.define.iter().collect::<Vec<_>>(),
        [
            ("__GCC_HAVE_SYNC_COMPARE_AND_SWAP_16", "1"),
            ("__SSE3__", "1"),
            ("__nocona", "1"),
            ("__nocona__", "1"),
            ("__tune_nocona__", "1"),
        ]
    );
    assert_eq!(join(c.undef), "__k8,__k8__,__tune_k8__");

    let c = t::cpu("skylake-avx512").unwrap();
    assert_eq!(
        join(c.features),
        "adx,aes,avx,avx2,avx512bw,avx512cd,avx512dq,avx512f,avx512vl,bmi,bmi2,clflushopt,clwb,\
         cmov,crc32,cx16,cx8,evex512,f16c,fma,fsgsbase,fxsr,invpcid,lzcnt,mmx,movbe,pclmul,pku,\
         popcnt,prfchw,rdrnd,rdseed,sahf,sse,sse2,sse3,sse4.1,sse4.2,ssse3,x87,xsave,xsavec,\
         xsaveopt,xsaves"
    );
    let define: Vec<(&str, &str)> = c.define.iter().collect();
    assert_eq!(define.len(), 41);
    assert_eq!(define[..3], [("__ADX__", "1"), ("__AES__", "1"), ("__AVX2__", "1")]);
    let names: Vec<&str> = define[35..].iter().map(|d| d.0).collect();
    assert_eq!(
        names,
        ["__XSAVEOPT__", "__XSAVES__", "__XSAVE__", "__corei7", "__corei7__", "__tune_corei7__"]
    );
    assert_eq!(join(c.undef), "__k8,__k8__,__tune_k8__");

    let c = t::cpu("diamondrapids").unwrap();
    assert_eq!((c.features.len(), c.define.len()), (104, 102));
    assert!(c.features.contains("amx-avx512") && !c.features.contains("sse4a"));
}

#[test]
fn every_implication_decodes() {
    let all: Vec<(&str, t::List)> = t::implications().collect();
    assert_eq!(all.len(), 118, "spec/landed/driver/WP-630.md counts 118 features");
    let names: Vec<&str> = all.iter().map(|e| e.0).collect();
    assert!(sorted(&names), "sorted by name, distinct");
    for (f, l) in &all {
        assert_eq!(t::implies(f).map(join), Some(join(*l)), "{f}");
        let imp: Vec<&str> = l.iter().collect();
        assert!(sorted(&imp) && l.contains(f), "{f}: sorted, itself included");
        for g in imp {
            assert!(t::implies(g).is_some(), "{f}: {g} is in the table (closed)");
        }
    }
    let imp = |f| t::implies(f).map(join);
    assert_eq!(imp("adx").as_deref(), Some("adx"));
    assert_eq!(imp("avx").as_deref(), Some("avx,sse,sse2,sse3,sse4.1,sse4.2,ssse3"));
    assert_eq!(
        imp("avx512f").as_deref(),
        Some("avx,avx2,avx512f,f16c,fma,sse,sse2,sse3,sse4.1,sse4.2,ssse3")
    );
    assert_eq!(imp("sse4.2").as_deref(), Some("sse,sse2,sse3,sse4.1,sse4.2,ssse3"));
    for f in ["cmov", "evex512", "zu"] {
        assert_eq!(imp(f).as_deref(), Some(f));
    }
    for f in ["", "no-sse", "sse5", "zz"] {
        assert!(t::implies(f).is_none(), "{f}");
    }
}
