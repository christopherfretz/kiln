//! [`Snap`] for the standard library's types and hashbrown's maps
//! (spec/snap.md "Standard impls").

use std::any::TypeId;
use std::borrow::Cow;
use std::cell::{Cell, OnceCell, RefCell};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use std::hash::{BuildHasher, Hash};
use std::marker::PhantomData;
use std::num::NonZero;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{
    AtomicBool, AtomicI8, AtomicI16, AtomicI32, AtomicI64, AtomicIsize, AtomicU8, AtomicU16,
    AtomicU32, AtomicU64, AtomicUsize, Ordering,
};
use std::sync::{Arc, Mutex, OnceLock, PoisonError, RwLock};

use crate::{Desc, ErrorKind, Reader, Schema, SchemaHash, Snap, SnapError, Writer, put_seq};

// ----- scalars ----------------------------------------------------------

macro_rules! int {
    ($($t:ty: $n:literal),* $(,)?) => {$(
        impl Snap for $t {
            const SIZE: Option<usize> = Some($n);
            #[inline]
            fn put(&self, w: &mut Writer) {
                w.bytes(&self.to_le_bytes());
            }
            #[inline]
            fn get(r: &mut Reader<'_>) -> Result<Self, SnapError> {
                r.array_of::<$n>(stringify!($t)).map(<$t>::from_le_bytes)
            }
            const SCHEMA_HASH: u64 = SchemaHash::new("int", stringify!($t)).finish();
            fn schema(s: &mut Schema) {
                s.register::<Self>(Desc::new("int").hash(Self::SCHEMA_HASH));
            }
        }
    )*};
}

int!(u8: 1, u16: 2, u32: 4, u64: 8, u128: 16, i8: 1, i16: 2, i32: 4, i64: 8, i128: 16);

macro_rules! word {
    ($($t:ty => $wide:ty),* $(,)?) => {$(
        /// Written as 8 bytes whatever the target; a value that does not
        /// fit the reading target is [`ErrorKind::BadValue`].
        impl Snap for $t {
            const SIZE: Option<usize> = Some(8);
            #[inline]
            fn put(&self, w: &mut Writer) {
                w.bytes(&(*self as $wide).to_le_bytes());
            }
            #[inline]
            fn get(r: &mut Reader<'_>) -> Result<Self, SnapError> {
                let at = r.offset();
                let v = <$wide>::from_le_bytes(r.array_of::<8>(stringify!($t))?);
                <$t>::try_from(v).map_err(|_| {
                    SnapError::new(ErrorKind::BadValue(stringify!($t)), stringify!($t), at)
                })
            }
            const SCHEMA_HASH: u64 = SchemaHash::new("int", stringify!($t)).finish();
            fn schema(s: &mut Schema) {
                s.register::<Self>(Desc::new("int").hash(Self::SCHEMA_HASH));
            }
        }
    )*};
}

word!(usize => u64, isize => i64);

macro_rules! float {
    ($($t:ty: $bits:ty),* $(,)?) => {$(
        /// The bit pattern (NaN payloads and signed zeros round-trip).
        impl Snap for $t {
            const SIZE: Option<usize> = <$bits as Snap>::SIZE;
            #[inline]
            fn put(&self, w: &mut Writer) {
                self.to_bits().put(w);
            }
            #[inline]
            fn get(r: &mut Reader<'_>) -> Result<Self, SnapError> {
                <$bits>::get(r).map(<$t>::from_bits)
            }
            const SCHEMA_HASH: u64 = SchemaHash::new("float", stringify!($t)).finish();
            fn schema(s: &mut Schema) {
                s.register::<Self>(Desc::new("float").hash(Self::SCHEMA_HASH));
            }
        }
    )*};
}

float!(f32: u32, f64: u64);

impl Snap for bool {
    const SIZE: Option<usize> = Some(1);
    #[inline]
    fn put(&self, w: &mut Writer) {
        w.u8(u8::from(*self));
    }
    #[inline]
    fn get(r: &mut Reader<'_>) -> Result<Self, SnapError> {
        match r.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(SnapError::new(ErrorKind::BadValue("bool"), "bool", r.offset() - 1)),
        }
    }
    const SCHEMA_HASH: u64 = SchemaHash::new("bool", "bool").finish();
    fn schema(s: &mut Schema) {
        s.register::<Self>(Desc::new("bool").hash(Self::SCHEMA_HASH));
    }
}

impl Snap for char {
    const SIZE: Option<usize> = Some(4);
    #[inline]
    fn put(&self, w: &mut Writer) {
        w.u32(u32::from(*self));
    }
    #[inline]
    fn get(r: &mut Reader<'_>) -> Result<Self, SnapError> {
        let v = r.u32()?;
        char::from_u32(v)
            .ok_or_else(|| SnapError::new(ErrorKind::BadValue("char"), "char", r.offset() - 4))
    }
    const SCHEMA_HASH: u64 = SchemaHash::new("char", "char").finish();
    fn schema(s: &mut Schema) {
        s.register::<Self>(Desc::new("char").hash(Self::SCHEMA_HASH));
    }
}

macro_rules! nonzero {
    ($($t:ty),* $(,)?) => {$(
        /// As the integer; `Option<NonZero<_>>` is the integer too, with 0
        /// for `None` (the niche).
        impl Snap for NonZero<$t> {
            const SIZE: Option<usize> = <$t as Snap>::SIZE;
            const OPTION_SIZE: Option<usize> = <$t as Snap>::SIZE;
            #[inline]
            fn put(&self, w: &mut Writer) {
                self.get().put(w);
            }
            #[inline]
            fn get(r: &mut Reader<'_>) -> Result<Self, SnapError> {
                let at = r.offset();
                NonZero::new(<$t>::get(r)?).ok_or_else(|| {
                    SnapError::new(ErrorKind::BadValue("zero NonZero"), stringify!(NonZero<$t>), at)
                })
            }
            const SCHEMA_HASH: u64 = SchemaHash::new("nonzero", "NonZero").field::<$t>("0").finish();
            const SHAPE_HASH: u64 = SchemaHash::new("nonzero", "NonZero").shape::<$t>("0").finish();
            fn schema(s: &mut Schema) {
                if s.register::<Self>(Desc::new("nonzero").hash(Self::SCHEMA_HASH).field::<$t>("0")) {
                    <$t>::schema(s);
                }
            }
            #[inline]
            fn put_option(v: Option<&Self>, w: &mut Writer) {
                v.map_or(0, |x| x.get()).put(w);
            }
            #[inline]
            fn get_option(r: &mut Reader<'_>) -> Result<Option<Self>, SnapError> {
                <$t>::get(r).map(NonZero::new)
            }
        }
    )*};
}

nonzero!(u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize);

impl Snap for () {
    const SIZE: Option<usize> = Some(0);
    #[inline]
    fn put(&self, _: &mut Writer) {}
    #[inline]
    fn get(_: &mut Reader<'_>) -> Result<Self, SnapError> {
        Ok(())
    }
    const SCHEMA_HASH: u64 = SchemaHash::new("unit", "()").finish();
    fn schema(s: &mut Schema) {
        s.register::<Self>(Desc::new("unit").hash(Self::SCHEMA_HASH));
    }
}

/// Nothing is written.
impl<T: ?Sized + 'static> Snap for PhantomData<T> {
    const SIZE: Option<usize> = Some(0);
    #[inline]
    fn put(&self, _: &mut Writer) {}
    #[inline]
    fn get(_: &mut Reader<'_>) -> Result<Self, SnapError> {
        Ok(PhantomData)
    }
    const SCHEMA_HASH: u64 = SchemaHash::new("phantom", "PhantomData").finish();
    fn schema(s: &mut Schema) {
        s.register::<Self>(Desc::new("phantom").hash(Self::SCHEMA_HASH));
    }
}

macro_rules! tuple {
    ($(($($n:tt $t:ident),+))*) => {$(
        impl<$($t: Snap),+> Snap for ($($t,)+) {
            const SIZE: Option<usize> = crate::__private::sum(&[$($t::SIZE),+]);
            #[inline]
            fn put(&self, w: &mut Writer) {
                $(self.$n.put(w);)+
            }
            #[inline]
            fn get(r: &mut Reader<'_>) -> Result<Self, SnapError> {
                Ok(($($t::get(r)?,)+))
            }
            const SCHEMA_HASH: u64 =
                SchemaHash::new("tuple", "tuple")$(.field::<$t>(stringify!($n)))+.finish();
            const SHAPE_HASH: u64 =
                SchemaHash::new("tuple", "tuple")$(.shape::<$t>(stringify!($n)))+.finish();
            fn schema(s: &mut Schema) {
                if s.register::<Self>(Desc::new("tuple").hash(Self::SCHEMA_HASH)$(.field::<$t>(stringify!($n)))+) {
                    $($t::schema(s);)+
                }
            }
        }
    )*};
}

tuple! {
    (0 A)
    (0 A, 1 B)
    (0 A, 1 B, 2 C)
    (0 A, 1 B, 2 C, 3 D)
    (0 A, 1 B, 2 C, 3 D, 4 E)
    (0 A, 1 B, 2 C, 3 D, 4 E, 5 F)
    (0 A, 1 B, 2 C, 3 D, 4 E, 5 F, 6 G)
    (0 A, 1 B, 2 C, 3 D, 4 E, 5 F, 6 G, 7 H)
}

/// The elements in order (no length).
impl<T: Snap, const N: usize> Snap for [T; N] {
    const SIZE: Option<usize> = match T::SIZE {
        Some(s) => Some(s * N),
        None => None,
    };
    #[inline]
    fn put(&self, w: &mut Writer) {
        for x in self {
            x.put(w);
        }
    }
    #[inline]
    fn get(r: &mut Reader<'_>) -> Result<Self, SnapError> {
        let mut err = None;
        let items: [Option<T>; N] = std::array::from_fn(|_| {
            if err.is_some() {
                return None;
            }
            T::get(r).map_err(|e| err = Some(e)).ok()
        });
        if let Some(e) = err {
            return Err(e);
        }
        Ok(items.map(|x| x.unwrap_or_else(|| unreachable!("every element decoded"))))
    }
    const SCHEMA_HASH: u64 =
        SchemaHash::new("array", "[T; N]").number(N as u64).field::<T>("item").finish();
    const SHAPE_HASH: u64 =
        SchemaHash::new("array", "[T; N]").number(N as u64).shape::<T>("item").finish();
    fn schema(s: &mut Schema) {
        if s.register::<Self>(
            Desc::new("array").hash(Self::SCHEMA_HASH).note(&N.to_string()).field::<T>("item"),
        ) {
            T::schema(s);
        }
    }
}

// ----- Option, Result, Box ----------------------------------------------

/// Through the element's [`Snap::put_option`] / [`Snap::get_option`]: a tag
/// byte (and a zero-padded `None` for a fixed-size `T`), or the niche.
impl<T: Snap> Snap for Option<T> {
    const SIZE: Option<usize> = T::OPTION_SIZE;
    #[inline]
    fn put(&self, w: &mut Writer) {
        T::put_option(self.as_ref(), w);
    }
    #[inline]
    fn get(r: &mut Reader<'_>) -> Result<Self, SnapError> {
        T::get_option(r)
    }
    const SCHEMA_HASH: u64 = SchemaHash::new("option", "Option").field::<T>("0").finish();
    const SHAPE_HASH: u64 = SchemaHash::new("option", "Option").shape::<T>("0").finish();
    fn schema(s: &mut Schema) {
        if s.register::<Self>(Desc::new("option").hash(Self::SCHEMA_HASH).field::<T>("0")) {
            T::schema(s);
        }
    }
}

impl<T: Snap, E: Snap> Snap for Result<T, E> {
    #[inline]
    fn put(&self, w: &mut Writer) {
        match self {
            Ok(v) => {
                w.u8(0);
                v.put(w);
            }
            Err(e) => {
                w.u8(1);
                e.put(w);
            }
        }
    }
    #[inline]
    fn get(r: &mut Reader<'_>) -> Result<Self, SnapError> {
        match r.u8()? {
            0 => T::get(r).map(Ok),
            1 => E::get(r).map(Err),
            t => Err(r.bad_tag::<Self>(u64::from(t), 1)),
        }
    }
    const SCHEMA_HASH: u64 =
        SchemaHash::new("result", "Result").field::<T>("ok").field::<E>("err").finish();
    const SHAPE_HASH: u64 =
        SchemaHash::new("result", "Result").shape::<T>("ok").shape::<E>("err").finish();
    fn schema(s: &mut Schema) {
        if s.register::<Self>(
            Desc::new("result").hash(Self::SCHEMA_HASH).field::<T>("ok").field::<E>("err"),
        ) {
            T::schema(s);
            E::schema(s);
        }
    }
}

/// The value; never fixed-size, so that a type recursive through `Box`
/// has a computable [`Snap::SIZE`].
impl<T: Snap> Snap for Box<T> {
    #[inline]
    fn put(&self, w: &mut Writer) {
        (**self).put(w);
    }
    #[inline]
    fn get(r: &mut Reader<'_>) -> Result<Self, SnapError> {
        T::get(r).map(Box::new)
    }
    const SCHEMA_HASH: u64 = SchemaHash::new("box", "Box").field::<T>("0").finish();
    const SHAPE_HASH: u64 = SchemaHash::new("box", "Box").shape::<T>("0").finish();
    fn schema(s: &mut Schema) {
        if s.register::<Self>(Desc::new("box").hash(Self::SCHEMA_HASH).field::<T>("0")) {
            T::schema(s);
        }
    }
}

// ----- sequences and strings --------------------------------------------

fn get_vec<T: Snap>(r: &mut Reader<'_>) -> Result<Vec<T>, SnapError> {
    let n = r.seq_len::<T>()?;
    let mut v = Vec::with_capacity(n);
    r.seq_items(n, |x| v.push(x))?;
    Ok(v)
}

/// The length, then the elements (the bulk path for a fixed-size `T`).
impl<T: Snap> Snap for Vec<T> {
    #[inline]
    fn put(&self, w: &mut Writer) {
        put_seq(w, self.len(), self);
    }
    #[inline]
    fn get(r: &mut Reader<'_>) -> Result<Self, SnapError> {
        get_vec(r)
    }
    const SCHEMA_HASH: u64 = SchemaHash::new("seq", "Vec").field::<T>("item").finish();
    const SHAPE_HASH: u64 = SchemaHash::new("seq", "Vec").shape::<T>("item").finish();
    fn schema(s: &mut Schema) {
        if s.register::<Self>(Desc::new("seq").hash(Self::SCHEMA_HASH).field::<T>("item")) {
            T::schema(s);
        }
    }
}

/// As `Vec<T>`.
impl<T: Snap> Snap for Box<[T]> {
    #[inline]
    fn put(&self, w: &mut Writer) {
        put_seq(w, self.len(), self.iter());
    }
    #[inline]
    fn get(r: &mut Reader<'_>) -> Result<Self, SnapError> {
        get_vec(r).map(Vec::into_boxed_slice)
    }
    const SCHEMA_HASH: u64 = SchemaHash::new("seq", "Box<[T]>").field::<T>("item").finish();
    const SHAPE_HASH: u64 = SchemaHash::new("seq", "Box<[T]>").shape::<T>("item").finish();
    fn schema(s: &mut Schema) {
        if s.register::<Self>(Desc::new("seq").hash(Self::SCHEMA_HASH).field::<T>("item")) {
            T::schema(s);
        }
    }
}

/// As `Vec<T>`, front to back.
impl<T: Snap> Snap for VecDeque<T> {
    #[inline]
    fn put(&self, w: &mut Writer) {
        put_seq(w, self.len(), self);
    }
    #[inline]
    fn get(r: &mut Reader<'_>) -> Result<Self, SnapError> {
        get_vec(r).map(VecDeque::from)
    }
    const SCHEMA_HASH: u64 = SchemaHash::new("seq", "VecDeque").field::<T>("item").finish();
    const SHAPE_HASH: u64 = SchemaHash::new("seq", "VecDeque").shape::<T>("item").finish();
    fn schema(s: &mut Schema) {
        if s.register::<Self>(Desc::new("seq").hash(Self::SCHEMA_HASH).field::<T>("item")) {
            T::schema(s);
        }
    }
}

/// The length, then the UTF-8 bytes (validated on read).
impl Snap for String {
    #[inline]
    fn put(&self, w: &mut Writer) {
        w.str(self);
    }
    #[inline]
    fn get(r: &mut Reader<'_>) -> Result<Self, SnapError> {
        r.str().map(str::to_owned)
    }
    const SCHEMA_HASH: u64 = SchemaHash::new("str", "String").finish();
    fn schema(s: &mut Schema) {
        s.register::<Self>(Desc::new("str").hash(Self::SCHEMA_HASH));
    }
}

/// As `String`.
impl Snap for Box<str> {
    #[inline]
    fn put(&self, w: &mut Writer) {
        w.str(self);
    }
    #[inline]
    fn get(r: &mut Reader<'_>) -> Result<Self, SnapError> {
        r.str().map(Box::from)
    }
    const SCHEMA_HASH: u64 = SchemaHash::new("str", "Box<str>").finish();
    fn schema(s: &mut Schema) {
        s.register::<Self>(Desc::new("str").hash(Self::SCHEMA_HASH));
    }
}

/// As `String`; read back as `Cow::Owned`.
impl Snap for Cow<'static, str> {
    #[inline]
    fn put(&self, w: &mut Writer) {
        w.str(self);
    }
    #[inline]
    fn get(r: &mut Reader<'_>) -> Result<Self, SnapError> {
        r.str().map(|s| Cow::Owned(s.to_owned()))
    }
    const SCHEMA_HASH: u64 = SchemaHash::new("str", "Cow<str>").finish();
    fn schema(s: &mut Schema) {
        s.register::<Self>(Desc::new("str").hash(Self::SCHEMA_HASH));
    }
}

/// As `Vec<T>`; read back as `Cow::Owned`.
impl<T: Snap + Clone> Snap for Cow<'static, [T]> {
    #[inline]
    fn put(&self, w: &mut Writer) {
        put_seq(w, self.len(), self.iter());
    }
    #[inline]
    fn get(r: &mut Reader<'_>) -> Result<Self, SnapError> {
        get_vec(r).map(Cow::Owned)
    }
    const SCHEMA_HASH: u64 = SchemaHash::new("seq", "Cow<[T]>").field::<T>("item").finish();
    const SHAPE_HASH: u64 = SchemaHash::new("seq", "Cow<[T]>").shape::<T>("item").finish();
    fn schema(s: &mut Schema) {
        if s.register::<Self>(Desc::new("seq").hash(Self::SCHEMA_HASH).field::<T>("item")) {
            T::schema(s);
        }
    }
}

/// The path's bytes (on Unix exactly; elsewhere as UTF-8, lossily).
impl Snap for PathBuf {
    #[inline]
    fn put(&self, w: &mut Writer) {
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt;
            let b = self.as_os_str().as_bytes();
            w.length(b.len());
            w.bytes(b);
        }
        #[cfg(not(unix))]
        w.str(&self.to_string_lossy());
    }
    #[inline]
    fn get(r: &mut Reader<'_>) -> Result<Self, SnapError> {
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStringExt;
            let n = r.seq_len::<u8>()?;
            let b = r.bytes(n)?;
            Ok(PathBuf::from(std::ffi::OsString::from_vec(b.to_vec())))
        }
        #[cfg(not(unix))]
        r.str().map(PathBuf::from)
    }
    const SCHEMA_HASH: u64 = SchemaHash::new("path", "PathBuf").finish();
    fn schema(s: &mut Schema) {
        s.register::<Self>(Desc::new("path").hash(Self::SCHEMA_HASH));
    }
}

// ----- interior mutability and synchronization --------------------------

/// The value.
impl<T: Snap + Copy> Snap for Cell<T> {
    const SIZE: Option<usize> = T::SIZE;
    #[inline]
    fn put(&self, w: &mut Writer) {
        self.get().put(w);
    }
    #[inline]
    fn get(r: &mut Reader<'_>) -> Result<Self, SnapError> {
        T::get(r).map(Cell::new)
    }
    const SCHEMA_HASH: u64 = SchemaHash::new("cell", "Cell").field::<T>("0").finish();
    const SHAPE_HASH: u64 = SchemaHash::new("cell", "Cell").shape::<T>("0").finish();
    fn schema(s: &mut Schema) {
        if s.register::<Self>(Desc::new("cell").hash(Self::SCHEMA_HASH).field::<T>("0")) {
            T::schema(s);
        }
    }
}

/// The value.
///
/// # Panics
/// `put` panics if the cell is mutably borrowed: a snapshot is taken at a
/// quiescent point (spec/snap.md "Hand-written roots").
impl<T: Snap> Snap for RefCell<T> {
    const SIZE: Option<usize> = T::SIZE;
    #[inline]
    fn put(&self, w: &mut Writer) {
        self.borrow().put(w);
    }
    #[inline]
    fn get(r: &mut Reader<'_>) -> Result<Self, SnapError> {
        T::get(r).map(RefCell::new)
    }
    const SCHEMA_HASH: u64 = SchemaHash::new("cell", "RefCell").field::<T>("0").finish();
    const SHAPE_HASH: u64 = SchemaHash::new("cell", "RefCell").shape::<T>("0").finish();
    fn schema(s: &mut Schema) {
        if s.register::<Self>(Desc::new("cell").hash(Self::SCHEMA_HASH).field::<T>("0")) {
            T::schema(s);
        }
    }
}

/// As `Option<T>`.
impl<T: Snap> Snap for OnceCell<T> {
    const SIZE: Option<usize> = T::OPTION_SIZE;
    #[inline]
    fn put(&self, w: &mut Writer) {
        T::put_option(self.get(), w);
    }
    #[inline]
    fn get(r: &mut Reader<'_>) -> Result<Self, SnapError> {
        Ok(T::get_option(r)?.map_or_else(OnceCell::new, OnceCell::from))
    }
    const SCHEMA_HASH: u64 = SchemaHash::new("once", "OnceCell").field::<T>("0").finish();
    const SHAPE_HASH: u64 = SchemaHash::new("once", "OnceCell").shape::<T>("0").finish();
    fn schema(s: &mut Schema) {
        if s.register::<Self>(Desc::new("once").hash(Self::SCHEMA_HASH).field::<T>("0")) {
            T::schema(s);
        }
    }
}

/// As `Option<T>`.
impl<T: Snap> Snap for OnceLock<T> {
    const SIZE: Option<usize> = T::OPTION_SIZE;
    #[inline]
    fn put(&self, w: &mut Writer) {
        T::put_option(self.get(), w);
    }
    #[inline]
    fn get(r: &mut Reader<'_>) -> Result<Self, SnapError> {
        Ok(T::get_option(r)?.map_or_else(OnceLock::new, OnceLock::from))
    }
    const SCHEMA_HASH: u64 = SchemaHash::new("once", "OnceLock").field::<T>("0").finish();
    const SHAPE_HASH: u64 = SchemaHash::new("once", "OnceLock").shape::<T>("0").finish();
    fn schema(s: &mut Schema) {
        if s.register::<Self>(Desc::new("once").hash(Self::SCHEMA_HASH).field::<T>("0")) {
            T::schema(s);
        }
    }
}

/// The inner value (a poisoned lock is read anyway). `put` locks, so the
/// writing thread must not hold the lock.
impl<T: Snap> Snap for Mutex<T> {
    const SIZE: Option<usize> = T::SIZE;
    #[inline]
    fn put(&self, w: &mut Writer) {
        self.lock().unwrap_or_else(PoisonError::into_inner).put(w);
    }
    #[inline]
    fn get(r: &mut Reader<'_>) -> Result<Self, SnapError> {
        T::get(r).map(Mutex::new)
    }
    const SCHEMA_HASH: u64 = SchemaHash::new("lock", "Mutex").field::<T>("0").finish();
    const SHAPE_HASH: u64 = SchemaHash::new("lock", "Mutex").shape::<T>("0").finish();
    fn schema(s: &mut Schema) {
        if s.register::<Self>(Desc::new("lock").hash(Self::SCHEMA_HASH).field::<T>("0")) {
            T::schema(s);
        }
    }
}

/// The inner value (a poisoned lock is read anyway). `put` takes the read
/// lock, so the writing thread must not hold the write lock.
impl<T: Snap> Snap for RwLock<T> {
    const SIZE: Option<usize> = T::SIZE;
    #[inline]
    fn put(&self, w: &mut Writer) {
        self.read().unwrap_or_else(PoisonError::into_inner).put(w);
    }
    #[inline]
    fn get(r: &mut Reader<'_>) -> Result<Self, SnapError> {
        T::get(r).map(RwLock::new)
    }
    const SCHEMA_HASH: u64 = SchemaHash::new("lock", "RwLock").field::<T>("0").finish();
    const SHAPE_HASH: u64 = SchemaHash::new("lock", "RwLock").shape::<T>("0").finish();
    fn schema(s: &mut Schema) {
        if s.register::<Self>(Desc::new("lock").hash(Self::SCHEMA_HASH).field::<T>("0")) {
            T::schema(s);
        }
    }
}

macro_rules! atomic {
    ($($a:ty: $t:ty),* $(,)?) => {$(
        /// The value (a `Relaxed` load).
        impl Snap for $a {
            const SIZE: Option<usize> = <$t as Snap>::SIZE;
            #[inline]
            fn put(&self, w: &mut Writer) {
                self.load(Ordering::Relaxed).put(w);
            }
            #[inline]
            fn get(r: &mut Reader<'_>) -> Result<Self, SnapError> {
                <$t>::get(r).map(<$a>::new)
            }
            const SCHEMA_HASH: u64 = SchemaHash::new("atomic", stringify!($a)).field::<$t>("0").finish();
            const SHAPE_HASH: u64 = SchemaHash::new("atomic", stringify!($a)).shape::<$t>("0").finish();
            fn schema(s: &mut Schema) {
                if s.register::<Self>(Desc::new("atomic").hash(Self::SCHEMA_HASH).field::<$t>("0")) {
                    <$t>::schema(s);
                }
            }
        }
    )*};
}

atomic!(
    AtomicBool: bool, AtomicU8: u8, AtomicU16: u16, AtomicU32: u32, AtomicU64: u64,
    AtomicUsize: usize, AtomicI8: i8, AtomicI16: i16, AtomicI32: i32, AtomicI64: i64,
    AtomicIsize: isize,
);

// ----- maps and sets ----------------------------------------------------

/// Writes `items` (an unordered table's `len` entries) sorted by their
/// encoded bytes: each is encoded alone in a scratch writer for its sort
/// key, then written for real in that order (so that shared objects are
/// numbered in the canonical order too).
fn put_sorted<I: Iterator>(
    w: &mut Writer,
    len: usize,
    items: I,
    put: impl Fn(&mut Writer, &I::Item),
) {
    let mut keyed: Vec<(Vec<u8>, I::Item)> = items
        .map(|x| {
            let mut s = w.scratch();
            put(&mut s, &x);
            (s.into_bytes(), x)
        })
        .collect();
    keyed.sort_unstable_by(|a, b| a.0.cmp(&b.0));
    w.length(len);
    w.length(len);
    for (_, x) in &keyed {
        put(w, x);
    }
}

/// The header of an unordered table in normal mode: its capacity and
/// length.
fn put_table_header(w: &mut Writer, capacity: usize, len: usize) {
    w.length(capacity);
    w.length(len);
}

/// Reads a table header: the capacity to reserve (capped by the input
/// left, and at least the length) and the entry count.
fn get_table_header<E: Snap>(r: &mut Reader<'_>) -> Result<(usize, usize), SnapError> {
    let cap = r.length()?;
    let n = r.seq_len::<E>()?;
    Ok((cap.min(r.remaining()).max(n), n))
}

fn duplicate<T: ?Sized>(r: &Reader<'_>) -> SnapError {
    r.invalid::<T>("duplicate key")
}

macro_rules! hash_map {
    ($($map:ident)::+) => {
        /// The capacity, the length, then the entries in iteration order;
        /// read by reserving that capacity and inserting in that order. In
        /// canonical mode, sorted by encoded bytes, with the length as the
        /// capacity.
        impl<K, V, S> Snap for $($map)::+<K, V, S>
        where
            K: Snap + Eq + Hash,
            V: Snap,
            S: BuildHasher + Default + 'static,
        {
            fn put(&self, w: &mut Writer) {
                if w.is_canonical() {
                    put_sorted(w, self.len(), self.iter(), |w, (k, v)| {
                        k.put(w);
                        v.put(w);
                    });
                } else {
                    put_table_header(w, self.capacity(), self.len());
                    for (k, v) in self {
                        k.put(w);
                        v.put(w);
                    }
                }
            }
            fn get(r: &mut Reader<'_>) -> Result<Self, SnapError> {
                let (cap, n) = get_table_header::<(K, V)>(r)?;
                let mut m = Self::with_capacity_and_hasher(cap, S::default());
                let mut dup = false;
                r.seq_items::<(K, V)>(n, |(k, v)| dup |= m.insert(k, v).is_some())?;
                if dup {
                    return Err(duplicate::<Self>(r));
                }
                Ok(m)
            }
            const SCHEMA_HASH: u64 = SchemaHash::new("map", stringify!($($map)::+)).field::<K>("key").field::<V>("value").finish();
            const SHAPE_HASH: u64 = SchemaHash::new("map", stringify!($($map)::+)).shape::<K>("key").shape::<V>("value").finish();
            fn schema(s: &mut Schema) {
                if s.register::<Self>(Desc::new("map").hash(Self::SCHEMA_HASH).field::<K>("key").field::<V>("value")) {
                    K::schema(s);
                    V::schema(s);
                }
            }
        }
    };
}

macro_rules! hash_set {
    ($($set:ident)::+) => {
        /// As the maps: capacity, length, elements in iteration order (or
        /// sorted by encoded bytes in canonical mode).
        impl<T, S> Snap for $($set)::+<T, S>
        where
            T: Snap + Eq + Hash,
            S: BuildHasher + Default + 'static,
        {
            fn put(&self, w: &mut Writer) {
                if w.is_canonical() {
                    put_sorted(w, self.len(), self.iter(), |w, x| x.put(w));
                } else {
                    put_table_header(w, self.capacity(), self.len());
                    for x in self {
                        x.put(w);
                    }
                }
            }
            fn get(r: &mut Reader<'_>) -> Result<Self, SnapError> {
                let (cap, n) = get_table_header::<T>(r)?;
                let mut m = Self::with_capacity_and_hasher(cap, S::default());
                let mut dup = false;
                r.seq_items::<T>(n, |x| dup |= !m.insert(x))?;
                if dup {
                    return Err(duplicate::<Self>(r));
                }
                Ok(m)
            }
            const SCHEMA_HASH: u64 = SchemaHash::new("set", stringify!($($set)::+)).field::<T>("item").finish();
            const SHAPE_HASH: u64 = SchemaHash::new("set", stringify!($($set)::+)).shape::<T>("item").finish();
            fn schema(s: &mut Schema) {
                if s.register::<Self>(Desc::new("set").hash(Self::SCHEMA_HASH).field::<T>("item")) {
                    T::schema(s);
                }
            }
        }
    };
}

hash_map!(HashMap);
hash_map!(hashbrown::HashMap);
hash_set!(HashSet);
hash_set!(hashbrown::HashSet);

// `hashbrown::HashTable<T>` has no impl: it stores no hasher, so a decoder
// cannot rebuild it generically. Its owner encodes the entries and
// re-inserts them with its own hash function (spec/snap.md "Standard
// impls").

/// The length, then the entries in key order.
impl<K: Snap + Ord, V: Snap> Snap for BTreeMap<K, V> {
    fn put(&self, w: &mut Writer) {
        w.length(self.len());
        for (k, v) in self {
            k.put(w);
            v.put(w);
        }
    }
    fn get(r: &mut Reader<'_>) -> Result<Self, SnapError> {
        let n = r.seq_len::<(K, V)>()?;
        let mut m = BTreeMap::new();
        let mut dup = false;
        r.seq_items::<(K, V)>(n, |(k, v)| dup |= m.insert(k, v).is_some())?;
        if dup {
            return Err(duplicate::<Self>(r));
        }
        Ok(m)
    }
    const SCHEMA_HASH: u64 =
        SchemaHash::new("ordmap", "BTreeMap").field::<K>("key").field::<V>("value").finish();
    const SHAPE_HASH: u64 =
        SchemaHash::new("ordmap", "BTreeMap").shape::<K>("key").shape::<V>("value").finish();
    fn schema(s: &mut Schema) {
        if s.register::<Self>(
            Desc::new("ordmap").hash(Self::SCHEMA_HASH).field::<K>("key").field::<V>("value"),
        ) {
            K::schema(s);
            V::schema(s);
        }
    }
}

/// The length, then the elements in order.
impl<T: Snap + Ord> Snap for BTreeSet<T> {
    fn put(&self, w: &mut Writer) {
        put_seq(w, self.len(), self);
    }
    fn get(r: &mut Reader<'_>) -> Result<Self, SnapError> {
        let n = r.seq_len::<T>()?;
        let mut m = BTreeSet::new();
        let mut dup = false;
        r.seq_items::<T>(n, |x| dup |= !m.insert(x))?;
        if dup {
            return Err(duplicate::<Self>(r));
        }
        Ok(m)
    }
    const SCHEMA_HASH: u64 = SchemaHash::new("ordset", "BTreeSet").field::<T>("item").finish();
    const SHAPE_HASH: u64 = SchemaHash::new("ordset", "BTreeSet").shape::<T>("item").finish();
    fn schema(s: &mut Schema) {
        if s.register::<Self>(Desc::new("ordset").hash(Self::SCHEMA_HASH).field::<T>("item")) {
            T::schema(s);
        }
    }
}

// ----- shared pointers --------------------------------------------------

/// Writes a shared allocation: `0` and the value the first time the writer
/// sees `addr` (as a `P`), else `index + 1`.
#[inline]
fn put_shared<P: 'static>(w: &mut Writer, addr: usize, inner: impl FnOnce(&mut Writer)) {
    if let Some(i) = w.share(addr, TypeId::of::<P>()) {
        w.length(i as usize + 1);
    } else {
        w.length(0);
        inner(w);
    }
}

/// Reads what [`put_shared`] wrote: a new object (decoded by `make` and
/// recorded under the next index) or a clone of a recorded one.
#[inline]
fn get_shared<P: Clone + 'static>(
    r: &mut Reader<'_>,
    make: impl FnOnce(&mut Reader<'_>) -> Result<P, SnapError>,
) -> Result<P, SnapError> {
    match r.length()? {
        0 => {
            let slot = r.shared_reserve();
            let p = make(r)?;
            r.shared_fill(slot, Box::new(p.clone()));
            Ok(p)
        }
        i => r.shared::<P>(i as u64 - 1),
    }
}

macro_rules! shared {
    ($($ptr:ident),*) => {$(
        /// Sharing preserved: each allocation is written once (keyed on
        /// its address) and later occurrences refer to it, so the reader
        /// hands out clones of one new allocation (`ptr_eq` holds where it
        /// held).
        impl<T: Snap> Snap for $ptr<T> {
            #[inline]
            fn put(&self, w: &mut Writer) {
                let addr = $ptr::as_ptr(self).addr();
                put_shared::<Self>(w, addr, |w| (**self).put(w));
            }
            #[inline]
            fn get(r: &mut Reader<'_>) -> Result<Self, SnapError> {
                get_shared(r, |r| T::get(r).map($ptr::new))
            }
            const SCHEMA_HASH: u64 = SchemaHash::new("shared", concat!(stringify!($ptr), "<T>")).field::<T>("0").finish();
            const SHAPE_HASH: u64 = SchemaHash::new("shared", concat!(stringify!($ptr), "<T>")).shape::<T>("0").finish();
            fn schema(s: &mut Schema) {
                if s.register::<Self>(Desc::new("shared").hash(Self::SCHEMA_HASH).field::<T>("0")) {
                    T::schema(s);
                }
            }
        }

        /// Sharing preserved, as for a sized `T`; the slice as `Vec<T>`.
        impl<T: Snap> Snap for $ptr<[T]> {
            #[inline]
            fn put(&self, w: &mut Writer) {
                let addr = $ptr::as_ptr(self).cast::<()>().addr();
                put_shared::<Self>(w, addr, |w| put_seq(w, self.len(), self.iter()));
            }
            #[inline]
            fn get(r: &mut Reader<'_>) -> Result<Self, SnapError> {
                get_shared(r, |r| get_vec(r).map($ptr::from))
            }
            const SCHEMA_HASH: u64 = SchemaHash::new("shared", concat!(stringify!($ptr), "<[T]>")).field::<Vec<T>>("0").finish();
            const SHAPE_HASH: u64 = SchemaHash::new("shared", concat!(stringify!($ptr), "<[T]>")).shape::<Vec<T>>("0").finish();
            fn schema(s: &mut Schema) {
                if s.register::<Self>(Desc::new("shared").hash(Self::SCHEMA_HASH).field::<Vec<T>>("0")) {
                    <Vec<T>>::schema(s);
                }
            }
        }

        /// Sharing preserved; the string as `String`.
        impl Snap for $ptr<str> {
            #[inline]
            fn put(&self, w: &mut Writer) {
                let addr = $ptr::as_ptr(self).cast::<()>().addr();
                put_shared::<Self>(w, addr, |w| w.str(self));
            }
            #[inline]
            fn get(r: &mut Reader<'_>) -> Result<Self, SnapError> {
                get_shared(r, |r| r.str().map($ptr::from))
            }
            const SCHEMA_HASH: u64 = SchemaHash::new("shared", concat!(stringify!($ptr), "<str>")).field::<String>("0").finish();
            const SHAPE_HASH: u64 = SchemaHash::new("shared", concat!(stringify!($ptr), "<str>")).shape::<String>("0").finish();
            fn schema(s: &mut Schema) {
                s.register::<Self>(Desc::new("shared").hash(Self::SCHEMA_HASH).field::<String>("0"));
            }
        }
    )*};
}

shared!(Rc, Arc);
