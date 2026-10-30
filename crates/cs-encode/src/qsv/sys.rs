//! Привязки к Intel® Video Processing Library (oneVPL), API 2.x.
//!
//! Сгенерированы `bindgen` 0.73 из заголовков `api/vpl` (intel/libvpl, тег v2.15.0) для цели
//! x86_64-pc-windows-msvc; размеры и смещения структур проверяются при компиляции. Функции не
//! импортируются: они берутся из динамически загруженного диспетчера `libvpl.dll` (ставит драйвер
//! Intel) — сигнатуры внизу файла.
//!
//! Заголовки, из которых получены привязки, распространяются по лицензии MIT:
//!
//! Copyright (c) 2020 Intel Corporation
//!
//! Permission is hereby granted, free of charge, to any person obtaining a copy of this software
//! and associated documentation files (the "Software"), to deal in the Software without
//! restriction, including without limitation the rights to use, copy, modify, merge, publish,
//! distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the
//! Software is furnished to do so, subject to the following conditions:
//!
//! The above copyright notice and this permission notice shall be included in all copies or
//! substantial portions of the Software.
//!
//! THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING
//! BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
//! NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
//! DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
//! OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

#![allow(
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    dead_code,
    unsafe_op_in_unsafe_fn,
    clippy::all,
    clippy::pedantic
)]

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct __BindgenBitfieldUnit<Storage> {
    storage: Storage,
}
impl<Storage> __BindgenBitfieldUnit<Storage> {
    #[inline]
    pub const fn new(storage: Storage) -> Self {
        Self { storage }
    }
}
impl<Storage> __BindgenBitfieldUnit<Storage>
where
    Storage: AsRef<[u8]> + AsMut<[u8]>,
{
    #[inline]
    fn extract_bit(byte: u8, index: usize) -> bool {
        let bit_index = if cfg!(target_endian = "big") { 7 - (index % 8) } else { index % 8 };
        let mask = 1 << bit_index;
        byte & mask == mask
    }
    #[inline]
    pub fn get_bit(&self, index: usize) -> bool {
        debug_assert!(index / 8 < self.storage.as_ref().len());
        let byte_index = index / 8;
        let byte = self.storage.as_ref()[byte_index];
        Self::extract_bit(byte, index)
    }
    #[inline]
    pub unsafe fn raw_get_bit(this: *const Self, index: usize) -> bool {
        debug_assert!(index / 8 < core::mem::size_of::<Storage>());
        let byte_index = index / 8;
        let byte = unsafe { *(core::ptr::addr_of!((*this).storage) as *const u8).offset(byte_index as isize) };
        Self::extract_bit(byte, index)
    }
    #[inline]
    fn change_bit(byte: u8, index: usize, val: bool) -> u8 {
        let bit_index = if cfg!(target_endian = "big") { 7 - (index % 8) } else { index % 8 };
        let mask = 1 << bit_index;
        if val { byte | mask } else { byte & !mask }
    }
    #[inline]
    pub fn set_bit(&mut self, index: usize, val: bool) {
        debug_assert!(index / 8 < self.storage.as_ref().len());
        let byte_index = index / 8;
        let byte = &mut self.storage.as_mut()[byte_index];
        *byte = Self::change_bit(*byte, index, val);
    }
    #[inline]
    pub unsafe fn raw_set_bit(this: *mut Self, index: usize, val: bool) {
        debug_assert!(index / 8 < core::mem::size_of::<Storage>());
        let byte_index = index / 8;
        let byte = unsafe { (core::ptr::addr_of_mut!((*this).storage) as *mut u8).offset(byte_index as isize) };
        unsafe { *byte = Self::change_bit(*byte, index, val) };
    }
    #[inline]
    pub fn get(&self, bit_offset: usize, bit_width: u8) -> u64 {
        debug_assert!(bit_width <= 64);
        debug_assert!(bit_offset / 8 < self.storage.as_ref().len());
        debug_assert!((bit_offset + (bit_width as usize) + 7) / 8 <= self.storage.as_ref().len());
        if bit_width == 0 {
            return 0;
        }
        let mut val = 0u64;
        let storage = self.storage.as_ref();
        let start_byte = bit_offset / 8;
        let bit_shift = bit_offset % 8;
        let bytes_needed = (bit_width as usize + bit_shift + 7) / 8;
        if cfg!(target_endian = "big") {
            for i in 0..bytes_needed {
                val |= (storage[start_byte + i].reverse_bits() as u64) << (i * 8);
            }
        } else {
            for i in 0..bytes_needed {
                val |= (storage[start_byte + i] as u64) << (i * 8);
            }
        }
        val >>= bit_shift;
        if bit_width < 64 {
            val &= (1u64 << bit_width) - 1;
        }
        if cfg!(target_endian = "big") {
            val = val.reverse_bits() >> (64 - bit_width as usize);
        }
        val
    }
    #[inline]
    pub unsafe fn raw_get(this: *const Self, bit_offset: usize, bit_width: u8) -> u64 {
        debug_assert!(bit_width <= 64);
        debug_assert!(bit_offset / 8 < core::mem::size_of::<Storage>());
        debug_assert!((bit_offset + (bit_width as usize) + 7) / 8 <= core::mem::size_of::<Storage>());
        if bit_width == 0 {
            return 0;
        }
        let mut val = 0u64;
        let start_byte = bit_offset / 8;
        let bit_shift = bit_offset % 8;
        let bytes_needed = (bit_width as usize + bit_shift + 7) / 8;
        let storage_ptr = unsafe { core::ptr::addr_of!((*this).storage) as *const u8 };
        if cfg!(target_endian = "big") {
            for i in 0..bytes_needed {
                let byte = unsafe { *storage_ptr.add(start_byte + i) };
                val |= (byte.reverse_bits() as u64) << (i * 8);
            }
        } else {
            for i in 0..bytes_needed {
                let byte = unsafe { *storage_ptr.add(start_byte + i) };
                val |= (byte as u64) << (i * 8);
            }
        }
        val >>= bit_shift;
        if bit_width < 64 {
            val &= (1u64 << bit_width) - 1;
        }
        if cfg!(target_endian = "big") {
            val = val.reverse_bits() >> (64 - bit_width as usize);
        }
        val
    }
    #[inline]
    pub fn set(&mut self, bit_offset: usize, bit_width: u8, val: u64) {
        debug_assert!(bit_width <= 64);
        debug_assert!(bit_offset / 8 < self.storage.as_ref().len());
        debug_assert!((bit_offset + (bit_width as usize) + 7) / 8 <= self.storage.as_ref().len());
        if bit_width == 0 {
            return;
        }
        let mut val = val;
        if bit_width < 64 {
            val &= (1u64 << bit_width) - 1;
        }
        if cfg!(target_endian = "big") {
            val = val.reverse_bits() >> (64 - bit_width as usize);
        }
        let storage = self.storage.as_mut();
        let start_byte = bit_offset / 8;
        let bit_shift = bit_offset % 8;
        let bytes_needed = (bit_width as usize + bit_shift + 7) / 8;
        val <<= bit_shift;
        let field_mask = if bit_width as usize + bit_shift >= 64 {
            !0u64 << bit_shift
        } else {
            ((1u64 << bit_width) - 1) << bit_shift
        };
        for i in 0..bytes_needed {
            let byte_val = (val >> (i * 8)) as u8;
            let byte_mask = (field_mask >> (i * 8)) as u8;
            if cfg!(target_endian = "big") {
                let byte = storage[start_byte + i].reverse_bits();
                let new_byte = (byte & !byte_mask) | (byte_val & byte_mask);
                storage[start_byte + i] = new_byte.reverse_bits();
            } else {
                storage[start_byte + i] = (storage[start_byte + i] & !byte_mask) | (byte_val & byte_mask);
            }
        }
    }
    #[inline]
    pub unsafe fn raw_set(this: *mut Self, bit_offset: usize, bit_width: u8, val: u64) {
        debug_assert!(bit_width <= 64);
        debug_assert!(bit_offset / 8 < core::mem::size_of::<Storage>());
        debug_assert!((bit_offset + (bit_width as usize) + 7) / 8 <= core::mem::size_of::<Storage>());
        if bit_width == 0 {
            return;
        }
        let mut val = val;
        if bit_width < 64 {
            val &= (1u64 << bit_width) - 1;
        }
        if cfg!(target_endian = "big") {
            val = val.reverse_bits() >> (64 - bit_width as usize);
        }
        let start_byte = bit_offset / 8;
        let bit_shift = bit_offset % 8;
        let bytes_needed = (bit_width as usize + bit_shift + 7) / 8;
        val <<= bit_shift;
        let field_mask = if bit_width as usize + bit_shift >= 64 {
            !0u64 << bit_shift
        } else {
            ((1u64 << bit_width) - 1) << bit_shift
        };
        let storage_ptr = unsafe { core::ptr::addr_of_mut!((*this).storage) as *mut u8 };
        for i in 0..bytes_needed {
            let byte_val = (val >> (i * 8)) as u8;
            let byte_mask = (field_mask >> (i * 8)) as u8;
            let byte_ptr = unsafe { storage_ptr.add(start_byte + i) };
            if cfg!(target_endian = "big") {
                let byte = unsafe { (*byte_ptr).reverse_bits() };
                let new_byte = (byte & !byte_mask) | (byte_val & byte_mask);
                unsafe { *byte_ptr = new_byte.reverse_bits() };
            } else {
                unsafe { *byte_ptr = (*byte_ptr & !byte_mask) | (byte_val & byte_mask) };
            }
        }
    }
}
#[doc = " Const-generic methods for efficient bitfield access when offset and width"]
#[doc = " are known at compile time."]
impl<const N: usize> __BindgenBitfieldUnit<[u8; N]> {
    #[doc = " Get a field using const generics for compile-time optimization."]
    #[doc = " Uses native word size operations when the field fits in usize."]
    #[inline]
    pub const fn get_const<const BIT_OFFSET: usize, const BIT_WIDTH: u8>(&self) -> u64 {
        debug_assert!(BIT_WIDTH <= 64);
        debug_assert!(BIT_OFFSET / 8 < N);
        debug_assert!((BIT_OFFSET + (BIT_WIDTH as usize) + 7) / 8 <= N);
        if BIT_WIDTH == 0 {
            return 0;
        }
        let start_byte = BIT_OFFSET / 8;
        let bit_shift = BIT_OFFSET % 8;
        let bytes_needed = (BIT_WIDTH as usize + bit_shift + 7) / 8;
        if BIT_WIDTH as usize + bit_shift <= usize::BITS as usize {
            let mut val = 0usize;
            if cfg!(target_endian = "big") {
                let mut i = 0;
                while i < bytes_needed {
                    val |= (self.storage[start_byte + i].reverse_bits() as usize) << (i * 8);
                    i += 1;
                }
            } else {
                let mut i = 0;
                while i < bytes_needed {
                    val |= (self.storage[start_byte + i] as usize) << (i * 8);
                    i += 1;
                }
            }
            val >>= bit_shift;
            if (BIT_WIDTH as u32) < usize::BITS {
                val &= (1usize << BIT_WIDTH) - 1;
            }
            if cfg!(target_endian = "big") {
                val = val.reverse_bits() >> (usize::BITS as usize - BIT_WIDTH as usize);
            }
            val as u64
        } else {
            let mut val = 0u64;
            if cfg!(target_endian = "big") {
                let mut i = 0;
                while i < bytes_needed {
                    val |= (self.storage[start_byte + i].reverse_bits() as u64) << (i * 8);
                    i += 1;
                }
            } else {
                let mut i = 0;
                while i < bytes_needed {
                    val |= (self.storage[start_byte + i] as u64) << (i * 8);
                    i += 1;
                }
            }
            val >>= bit_shift;
            if BIT_WIDTH < 64 {
                val &= (1u64 << BIT_WIDTH) - 1;
            }
            if cfg!(target_endian = "big") {
                val = val.reverse_bits() >> (64 - BIT_WIDTH as usize);
            }
            val
        }
    }
    #[doc = " Set a field using const generics for compile-time optimization."]
    #[doc = " Uses native word size operations when the field fits in usize."]
    #[inline]
    pub fn set_const<const BIT_OFFSET: usize, const BIT_WIDTH: u8>(&mut self, val: u64) {
        debug_assert!(BIT_WIDTH <= 64);
        debug_assert!(BIT_OFFSET / 8 < N);
        debug_assert!((BIT_OFFSET + (BIT_WIDTH as usize) + 7) / 8 <= N);
        if BIT_WIDTH == 0 {
            return;
        }
        let start_byte = BIT_OFFSET / 8;
        let bit_shift = BIT_OFFSET % 8;
        let bytes_needed = (BIT_WIDTH as usize + bit_shift + 7) / 8;
        if BIT_WIDTH as usize + bit_shift <= usize::BITS as usize {
            let mut val = val as usize;
            if (BIT_WIDTH as u32) < usize::BITS {
                val &= (1usize << BIT_WIDTH) - 1;
            }
            if cfg!(target_endian = "big") {
                val = val.reverse_bits() >> (usize::BITS as usize - BIT_WIDTH as usize);
            }
            val <<= bit_shift;
            let field_mask = if BIT_WIDTH as usize + bit_shift >= usize::BITS as usize {
                !0usize << bit_shift
            } else {
                ((1usize << BIT_WIDTH) - 1) << bit_shift
            };
            let mut i = 0;
            while i < bytes_needed {
                let byte_val = (val >> (i * 8)) as u8;
                let byte_mask = (field_mask >> (i * 8)) as u8;
                if cfg!(target_endian = "big") {
                    let byte = self.storage[start_byte + i].reverse_bits();
                    let new_byte = (byte & !byte_mask) | (byte_val & byte_mask);
                    self.storage[start_byte + i] = new_byte.reverse_bits();
                } else {
                    self.storage[start_byte + i] = (self.storage[start_byte + i] & !byte_mask) | (byte_val & byte_mask);
                }
                i += 1;
            }
        } else {
            let mut val = val;
            if BIT_WIDTH < 64 {
                val &= (1u64 << BIT_WIDTH) - 1;
            }
            if cfg!(target_endian = "big") {
                val = val.reverse_bits() >> (64 - BIT_WIDTH as usize);
            }
            val <<= bit_shift;
            let field_mask = if BIT_WIDTH as usize + bit_shift >= 64 {
                !0u64 << bit_shift
            } else {
                ((1u64 << BIT_WIDTH) - 1) << bit_shift
            };
            let mut i = 0;
            while i < bytes_needed {
                let byte_val = (val >> (i * 8)) as u8;
                let byte_mask = (field_mask >> (i * 8)) as u8;
                if cfg!(target_endian = "big") {
                    let byte = self.storage[start_byte + i].reverse_bits();
                    let new_byte = (byte & !byte_mask) | (byte_val & byte_mask);
                    self.storage[start_byte + i] = new_byte.reverse_bits();
                } else {
                    self.storage[start_byte + i] = (self.storage[start_byte + i] & !byte_mask) | (byte_val & byte_mask);
                }
                i += 1;
            }
        }
    }
    #[doc = " Raw pointer get using const generics for compile-time optimization."]
    #[doc = " Uses native word size operations when the field fits in usize."]
    #[inline]
    pub const unsafe fn raw_get_const<const BIT_OFFSET: usize, const BIT_WIDTH: u8>(this: *const Self) -> u64 {
        debug_assert!(BIT_WIDTH <= 64);
        debug_assert!(BIT_OFFSET / 8 < N);
        debug_assert!((BIT_OFFSET + (BIT_WIDTH as usize) + 7) / 8 <= N);
        if BIT_WIDTH == 0 {
            return 0;
        }
        let start_byte = BIT_OFFSET / 8;
        let bit_shift = BIT_OFFSET % 8;
        let bytes_needed = (BIT_WIDTH as usize + bit_shift + 7) / 8;
        let storage_ptr = unsafe { core::ptr::addr_of!((*this).storage) as *const u8 };
        if BIT_WIDTH as usize + bit_shift <= usize::BITS as usize {
            let mut val = 0usize;
            if cfg!(target_endian = "big") {
                let mut i = 0;
                while i < bytes_needed {
                    let byte = unsafe { *storage_ptr.add(start_byte + i) };
                    val |= (byte.reverse_bits() as usize) << (i * 8);
                    i += 1;
                }
            } else {
                let mut i = 0;
                while i < bytes_needed {
                    let byte = unsafe { *storage_ptr.add(start_byte + i) };
                    val |= (byte as usize) << (i * 8);
                    i += 1;
                }
            }
            val >>= bit_shift;
            if (BIT_WIDTH as u32) < usize::BITS {
                val &= (1usize << BIT_WIDTH) - 1;
            }
            if cfg!(target_endian = "big") {
                val = val.reverse_bits() >> (usize::BITS as usize - BIT_WIDTH as usize);
            }
            val as u64
        } else {
            let mut val = 0u64;
            if cfg!(target_endian = "big") {
                let mut i = 0;
                while i < bytes_needed {
                    let byte = unsafe { *storage_ptr.add(start_byte + i) };
                    val |= (byte.reverse_bits() as u64) << (i * 8);
                    i += 1;
                }
            } else {
                let mut i = 0;
                while i < bytes_needed {
                    let byte = unsafe { *storage_ptr.add(start_byte + i) };
                    val |= (byte as u64) << (i * 8);
                    i += 1;
                }
            }
            val >>= bit_shift;
            if BIT_WIDTH < 64 {
                val &= (1u64 << BIT_WIDTH) - 1;
            }
            if cfg!(target_endian = "big") {
                val = val.reverse_bits() >> (64 - BIT_WIDTH as usize);
            }
            val
        }
    }
    #[doc = " Raw pointer set using const generics for compile-time optimization."]
    #[doc = " Uses native word size operations when the field fits in usize."]
    #[inline]
    pub unsafe fn raw_set_const<const BIT_OFFSET: usize, const BIT_WIDTH: u8>(this: *mut Self, val: u64) {
        debug_assert!(BIT_WIDTH <= 64);
        debug_assert!(BIT_OFFSET / 8 < N);
        debug_assert!((BIT_OFFSET + (BIT_WIDTH as usize) + 7) / 8 <= N);
        if BIT_WIDTH == 0 {
            return;
        }
        let start_byte = BIT_OFFSET / 8;
        let bit_shift = BIT_OFFSET % 8;
        let bytes_needed = (BIT_WIDTH as usize + bit_shift + 7) / 8;
        let storage_ptr = this.cast::<[u8; N]>().cast::<u8>();
        if BIT_WIDTH as usize + bit_shift <= usize::BITS as usize {
            let mut val = val as usize;
            if (BIT_WIDTH as u32) < usize::BITS {
                val &= (1usize << BIT_WIDTH) - 1;
            }
            if cfg!(target_endian = "big") {
                val = val.reverse_bits() >> (usize::BITS as usize - BIT_WIDTH as usize);
            }
            val <<= bit_shift;
            let field_mask = if BIT_WIDTH as usize + bit_shift >= usize::BITS as usize {
                !0usize << bit_shift
            } else {
                ((1usize << BIT_WIDTH) - 1) << bit_shift
            };
            let mut i = 0;
            while i < bytes_needed {
                let byte_val = (val >> (i * 8)) as u8;
                let byte_mask = (field_mask >> (i * 8)) as u8;
                let byte_ptr = unsafe { storage_ptr.add(start_byte + i) };
                if cfg!(target_endian = "big") {
                    let byte = unsafe { (*byte_ptr).reverse_bits() };
                    let new_byte = (byte & !byte_mask) | (byte_val & byte_mask);
                    unsafe { *byte_ptr = new_byte.reverse_bits() };
                } else {
                    unsafe { *byte_ptr = (*byte_ptr & !byte_mask) | (byte_val & byte_mask) };
                }
                i += 1;
            }
        } else {
            let mut val = val;
            if BIT_WIDTH < 64 {
                val &= (1u64 << BIT_WIDTH) - 1;
            }
            if cfg!(target_endian = "big") {
                val = val.reverse_bits() >> (64 - BIT_WIDTH as usize);
            }
            val <<= bit_shift;
            let field_mask = if BIT_WIDTH as usize + bit_shift >= 64 {
                !0u64 << bit_shift
            } else {
                ((1u64 << BIT_WIDTH) - 1) << bit_shift
            };
            let mut i = 0;
            while i < bytes_needed {
                let byte_val = (val >> (i * 8)) as u8;
                let byte_mask = (field_mask >> (i * 8)) as u8;
                let byte_ptr = unsafe { storage_ptr.add(start_byte + i) };
                if cfg!(target_endian = "big") {
                    let byte = unsafe { (*byte_ptr).reverse_bits() };
                    let new_byte = (byte & !byte_mask) | (byte_val & byte_mask);
                    unsafe { *byte_ptr = new_byte.reverse_bits() };
                } else {
                    unsafe { *byte_ptr = (*byte_ptr & !byte_mask) | (byte_val & byte_mask) };
                }
                i += 1;
            }
        }
    }
}
pub const MFX_VERSION_MAJOR: u32 = 2;
pub const MFX_VERSION_MINOR: u32 = 15;
pub const MFX_VERSION: u32 = 2015;
pub const MFX_LEGACY_VERSION: u32 = 1035;
pub const MFX_INFINITE: u32 = 4294967295;
pub const MFX_IMPL_NAME_LEN: u32 = 32;
pub const MFX_STRFIELD_LEN: u32 = 128;
pub type mfxU8 = ::core::ffi::c_uchar;
pub type mfxI8 = ::core::ffi::c_char;
pub type mfxI16 = ::core::ffi::c_short;
pub type mfxU16 = ::core::ffi::c_ushort;
pub type mfxU32 = ::core::ffi::c_uint;
pub type mfxI32 = ::core::ffi::c_int;
pub type mfxF32 = f32;
pub type mfxF64 = f64;
pub type mfxU64 = ::core::ffi::c_ulonglong;
pub type mfxI64 = ::core::ffi::c_longlong;
pub type mfxHDL = *mut ::core::ffi::c_void;
pub type mfxMemId = mfxHDL;
pub type mfxChar = ::core::ffi::c_char;
pub type mfxFP16 = ::core::ffi::c_ushort;
#[repr(C)]
#[derive(Copy, Clone)]
pub union mfxStructVersion {
    pub __bindgen_anon_1: mfxStructVersion__bindgen_ty_1,
    pub Version: mfxU16,
}
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct mfxStructVersion__bindgen_ty_1 {
    pub Minor: mfxU8,
    pub Major: mfxU8,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of mfxStructVersion__bindgen_ty_1"][::core::mem::size_of::<mfxStructVersion__bindgen_ty_1>() - 2usize];
    ["Alignment of mfxStructVersion__bindgen_ty_1"][::core::mem::align_of::<mfxStructVersion__bindgen_ty_1>() - 1usize];
    ["Offset of field: mfxStructVersion__bindgen_ty_1::Minor"]
        [::core::mem::offset_of!(mfxStructVersion__bindgen_ty_1, Minor) - 0usize];
    ["Offset of field: mfxStructVersion__bindgen_ty_1::Major"]
        [::core::mem::offset_of!(mfxStructVersion__bindgen_ty_1, Major) - 1usize];
};
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of mfxStructVersion"][::core::mem::size_of::<mfxStructVersion>() - 2usize];
    ["Alignment of mfxStructVersion"][::core::mem::align_of::<mfxStructVersion>() - 2usize];
    ["Offset of field: mfxStructVersion::Version"][::core::mem::offset_of!(mfxStructVersion, Version) - 0usize];
};
pub const MFX_VARIANT_TYPE_UNSET: mfxVariantType = 0;
pub const MFX_VARIANT_TYPE_U8: mfxVariantType = 1;
pub const MFX_VARIANT_TYPE_I8: mfxVariantType = 2;
pub const MFX_VARIANT_TYPE_U16: mfxVariantType = 3;
pub const MFX_VARIANT_TYPE_I16: mfxVariantType = 4;
pub const MFX_VARIANT_TYPE_U32: mfxVariantType = 5;
pub const MFX_VARIANT_TYPE_I32: mfxVariantType = 6;
pub const MFX_VARIANT_TYPE_U64: mfxVariantType = 7;
pub const MFX_VARIANT_TYPE_I64: mfxVariantType = 8;
pub const MFX_VARIANT_TYPE_F32: mfxVariantType = 9;
pub const MFX_VARIANT_TYPE_F64: mfxVariantType = 10;
pub const MFX_VARIANT_TYPE_PTR: mfxVariantType = 11;
pub const MFX_VARIANT_TYPE_FP16: mfxVariantType = 12;
pub type mfxVariantType = ::core::ffi::c_int;
#[repr(C)]
#[derive(Copy, Clone)]
pub struct mfxVariant {
    pub Version: mfxStructVersion,
    pub Type: mfxVariantType,
    pub Data: mfxVariant_data,
}
#[repr(C)]
#[derive(Copy, Clone)]
pub union mfxVariant_data {
    pub U8: mfxU8,
    pub I8: mfxI8,
    pub U16: mfxU16,
    pub I16: mfxI16,
    pub U32: mfxU32,
    pub I32: mfxI32,
    pub U64: mfxU64,
    pub I64: mfxI64,
    pub F32: mfxF32,
    pub F64: mfxF64,
    pub FP16: mfxFP16,
    pub Ptr: mfxHDL,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of mfxVariant_data"][::core::mem::size_of::<mfxVariant_data>() - 8usize];
    ["Alignment of mfxVariant_data"][::core::mem::align_of::<mfxVariant_data>() - 8usize];
    ["Offset of field: mfxVariant_data::U8"][::core::mem::offset_of!(mfxVariant_data, U8) - 0usize];
    ["Offset of field: mfxVariant_data::I8"][::core::mem::offset_of!(mfxVariant_data, I8) - 0usize];
    ["Offset of field: mfxVariant_data::U16"][::core::mem::offset_of!(mfxVariant_data, U16) - 0usize];
    ["Offset of field: mfxVariant_data::I16"][::core::mem::offset_of!(mfxVariant_data, I16) - 0usize];
    ["Offset of field: mfxVariant_data::U32"][::core::mem::offset_of!(mfxVariant_data, U32) - 0usize];
    ["Offset of field: mfxVariant_data::I32"][::core::mem::offset_of!(mfxVariant_data, I32) - 0usize];
    ["Offset of field: mfxVariant_data::U64"][::core::mem::offset_of!(mfxVariant_data, U64) - 0usize];
    ["Offset of field: mfxVariant_data::I64"][::core::mem::offset_of!(mfxVariant_data, I64) - 0usize];
    ["Offset of field: mfxVariant_data::F32"][::core::mem::offset_of!(mfxVariant_data, F32) - 0usize];
    ["Offset of field: mfxVariant_data::F64"][::core::mem::offset_of!(mfxVariant_data, F64) - 0usize];
    ["Offset of field: mfxVariant_data::FP16"][::core::mem::offset_of!(mfxVariant_data, FP16) - 0usize];
    ["Offset of field: mfxVariant_data::Ptr"][::core::mem::offset_of!(mfxVariant_data, Ptr) - 0usize];
};
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of mfxVariant"][::core::mem::size_of::<mfxVariant>() - 16usize];
    ["Alignment of mfxVariant"][::core::mem::align_of::<mfxVariant>() - 8usize];
    ["Offset of field: mfxVariant::Version"][::core::mem::offset_of!(mfxVariant, Version) - 0usize];
    ["Offset of field: mfxVariant::Type"][::core::mem::offset_of!(mfxVariant, Type) - 4usize];
    ["Offset of field: mfxVariant::Data"][::core::mem::offset_of!(mfxVariant, Data) - 8usize];
};
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct mfxI16Pair {
    pub x: mfxI16,
    pub y: mfxI16,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of mfxI16Pair"][::core::mem::size_of::<mfxI16Pair>() - 4usize];
    ["Alignment of mfxI16Pair"][::core::mem::align_of::<mfxI16Pair>() - 2usize];
    ["Offset of field: mfxI16Pair::x"][::core::mem::offset_of!(mfxI16Pair, x) - 0usize];
    ["Offset of field: mfxI16Pair::y"][::core::mem::offset_of!(mfxI16Pair, y) - 2usize];
};
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct mfxHDLPair {
    pub first: mfxHDL,
    pub second: mfxHDL,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of mfxHDLPair"][::core::mem::size_of::<mfxHDLPair>() - 16usize];
    ["Alignment of mfxHDLPair"][::core::mem::align_of::<mfxHDLPair>() - 8usize];
    ["Offset of field: mfxHDLPair::first"][::core::mem::offset_of!(mfxHDLPair, first) - 0usize];
    ["Offset of field: mfxHDLPair::second"][::core::mem::offset_of!(mfxHDLPair, second) - 8usize];
};
pub const MFX_ERR_NONE: mfxStatus = 0;
pub const MFX_ERR_UNKNOWN: mfxStatus = -1;
pub const MFX_ERR_NULL_PTR: mfxStatus = -2;
pub const MFX_ERR_UNSUPPORTED: mfxStatus = -3;
pub const MFX_ERR_MEMORY_ALLOC: mfxStatus = -4;
pub const MFX_ERR_NOT_ENOUGH_BUFFER: mfxStatus = -5;
pub const MFX_ERR_INVALID_HANDLE: mfxStatus = -6;
pub const MFX_ERR_LOCK_MEMORY: mfxStatus = -7;
pub const MFX_ERR_NOT_INITIALIZED: mfxStatus = -8;
pub const MFX_ERR_NOT_FOUND: mfxStatus = -9;
pub const MFX_ERR_MORE_DATA: mfxStatus = -10;
pub const MFX_ERR_MORE_SURFACE: mfxStatus = -11;
pub const MFX_ERR_ABORTED: mfxStatus = -12;
pub const MFX_ERR_DEVICE_LOST: mfxStatus = -13;
pub const MFX_ERR_INCOMPATIBLE_VIDEO_PARAM: mfxStatus = -14;
pub const MFX_ERR_INVALID_VIDEO_PARAM: mfxStatus = -15;
pub const MFX_ERR_UNDEFINED_BEHAVIOR: mfxStatus = -16;
pub const MFX_ERR_DEVICE_FAILED: mfxStatus = -17;
pub const MFX_ERR_MORE_BITSTREAM: mfxStatus = -18;
pub const MFX_ERR_GPU_HANG: mfxStatus = -21;
pub const MFX_ERR_REALLOC_SURFACE: mfxStatus = -22;
pub const MFX_ERR_RESOURCE_MAPPED: mfxStatus = -23;
pub const MFX_ERR_NOT_IMPLEMENTED: mfxStatus = -24;
pub const MFX_ERR_MORE_EXTBUFFER: mfxStatus = -25;
pub const MFX_WRN_IN_EXECUTION: mfxStatus = 1;
pub const MFX_WRN_DEVICE_BUSY: mfxStatus = 2;
pub const MFX_WRN_VIDEO_PARAM_CHANGED: mfxStatus = 3;
pub const MFX_WRN_PARTIAL_ACCELERATION: mfxStatus = 4;
pub const MFX_WRN_INCOMPATIBLE_VIDEO_PARAM: mfxStatus = 5;
pub const MFX_WRN_VALUE_NOT_CHANGED: mfxStatus = 6;
pub const MFX_WRN_OUT_OF_RANGE: mfxStatus = 7;
pub const MFX_WRN_FILTER_SKIPPED: mfxStatus = 10;
pub const MFX_ERR_NONE_PARTIAL_OUTPUT: mfxStatus = 12;
pub const MFX_WRN_ALLOC_TIMEOUT_EXPIRED: mfxStatus = 13;
pub const MFX_TASK_DONE: mfxStatus = 0;
pub const MFX_TASK_WORKING: mfxStatus = 8;
pub const MFX_TASK_BUSY: mfxStatus = 9;
pub const MFX_ERR_MORE_DATA_SUBMIT_TASK: mfxStatus = -10000;
pub type mfxStatus = ::core::ffi::c_int;
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct mfxGUID {
    pub Data: [mfxU8; 16usize],
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of mfxGUID"][::core::mem::size_of::<mfxGUID>() - 16usize];
    ["Alignment of mfxGUID"][::core::mem::align_of::<mfxGUID>() - 1usize];
    ["Offset of field: mfxGUID::Data"][::core::mem::offset_of!(mfxGUID, Data) - 0usize];
};
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct mfxExtBuffer {
    pub BufferId: mfxU32,
    pub BufferSz: mfxU32,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of mfxExtBuffer"][::core::mem::size_of::<mfxExtBuffer>() - 8usize];
    ["Alignment of mfxExtBuffer"][::core::mem::align_of::<mfxExtBuffer>() - 4usize];
    ["Offset of field: mfxExtBuffer::BufferId"][::core::mem::offset_of!(mfxExtBuffer, BufferId) - 0usize];
    ["Offset of field: mfxExtBuffer::BufferSz"][::core::mem::offset_of!(mfxExtBuffer, BufferSz) - 4usize];
};
pub type mfxIMPL = mfxI32;
pub const MFX_IMPL_AUTO: _bindgen_ty_1 = 0;
pub const MFX_IMPL_SOFTWARE: _bindgen_ty_1 = 1;
pub const MFX_IMPL_HARDWARE: _bindgen_ty_1 = 2;
pub const MFX_IMPL_AUTO_ANY: _bindgen_ty_1 = 3;
pub const MFX_IMPL_HARDWARE_ANY: _bindgen_ty_1 = 4;
pub const MFX_IMPL_HARDWARE2: _bindgen_ty_1 = 5;
pub const MFX_IMPL_HARDWARE3: _bindgen_ty_1 = 6;
pub const MFX_IMPL_HARDWARE4: _bindgen_ty_1 = 7;
pub const MFX_IMPL_RUNTIME: _bindgen_ty_1 = 8;
pub const MFX_IMPL_VIA_ANY: _bindgen_ty_1 = 256;
pub const MFX_IMPL_VIA_D3D9: _bindgen_ty_1 = 512;
pub const MFX_IMPL_VIA_D3D11: _bindgen_ty_1 = 768;
pub const MFX_IMPL_VIA_VAAPI: _bindgen_ty_1 = 1024;
pub const MFX_IMPL_VIA_HDDLUNITE: _bindgen_ty_1 = 1280;
pub const MFX_IMPL_UNSUPPORTED: _bindgen_ty_1 = 0;
pub type _bindgen_ty_1 = ::core::ffi::c_int;
#[repr(C)]
#[derive(Copy, Clone)]
pub union mfxVersion {
    pub __bindgen_anon_1: mfxVersion__bindgen_ty_1,
    pub Version: mfxU32,
}
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct mfxVersion__bindgen_ty_1 {
    pub Minor: mfxU16,
    pub Major: mfxU16,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of mfxVersion__bindgen_ty_1"][::core::mem::size_of::<mfxVersion__bindgen_ty_1>() - 4usize];
    ["Alignment of mfxVersion__bindgen_ty_1"][::core::mem::align_of::<mfxVersion__bindgen_ty_1>() - 2usize];
    ["Offset of field: mfxVersion__bindgen_ty_1::Minor"]
        [::core::mem::offset_of!(mfxVersion__bindgen_ty_1, Minor) - 0usize];
    ["Offset of field: mfxVersion__bindgen_ty_1::Major"]
        [::core::mem::offset_of!(mfxVersion__bindgen_ty_1, Major) - 2usize];
};
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of mfxVersion"][::core::mem::size_of::<mfxVersion>() - 4usize];
    ["Alignment of mfxVersion"][::core::mem::align_of::<mfxVersion>() - 4usize];
    ["Offset of field: mfxVersion::Version"][::core::mem::offset_of!(mfxVersion, Version) - 0usize];
};
#[repr(C)]
#[derive(Debug)]
pub struct _mfxEncryptedData {
    _unused: [u8; 0],
}
pub type mfxEncryptedData = _mfxEncryptedData;
#[repr(C)]
#[derive(Copy, Clone)]
pub struct mfxBitstream {
    pub __bindgen_anon_1: mfxBitstream__bindgen_ty_1,
    pub DecodeTimeStamp: mfxI64,
    pub TimeStamp: mfxU64,
    pub Data: *mut mfxU8,
    pub DataOffset: mfxU32,
    pub DataLength: mfxU32,
    pub MaxLength: mfxU32,
    pub PicStruct: mfxU16,
    pub FrameType: mfxU16,
    pub DataFlag: mfxU16,
    pub reserved2: mfxU16,
}
#[repr(C)]
#[derive(Copy, Clone)]
pub union mfxBitstream__bindgen_ty_1 {
    pub __bindgen_anon_1: mfxBitstream__bindgen_ty_1__bindgen_ty_1,
    pub reserved: [mfxU32; 6usize],
}
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct mfxBitstream__bindgen_ty_1__bindgen_ty_1 {
    pub EncryptedData: *mut mfxEncryptedData,
    pub ExtParam: *mut *mut mfxExtBuffer,
    pub NumExtParam: mfxU16,
    pub reserved1: mfxU16,
    pub CodecId: mfxU32,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of mfxBitstream__bindgen_ty_1__bindgen_ty_1"]
        [::core::mem::size_of::<mfxBitstream__bindgen_ty_1__bindgen_ty_1>() - 24usize];
    ["Alignment of mfxBitstream__bindgen_ty_1__bindgen_ty_1"]
        [::core::mem::align_of::<mfxBitstream__bindgen_ty_1__bindgen_ty_1>() - 8usize];
    ["Offset of field: mfxBitstream__bindgen_ty_1__bindgen_ty_1::EncryptedData"]
        [::core::mem::offset_of!(mfxBitstream__bindgen_ty_1__bindgen_ty_1, EncryptedData) - 0usize];
    ["Offset of field: mfxBitstream__bindgen_ty_1__bindgen_ty_1::ExtParam"]
        [::core::mem::offset_of!(mfxBitstream__bindgen_ty_1__bindgen_ty_1, ExtParam) - 8usize];
    ["Offset of field: mfxBitstream__bindgen_ty_1__bindgen_ty_1::NumExtParam"]
        [::core::mem::offset_of!(mfxBitstream__bindgen_ty_1__bindgen_ty_1, NumExtParam) - 16usize];
    ["Offset of field: mfxBitstream__bindgen_ty_1__bindgen_ty_1::reserved1"]
        [::core::mem::offset_of!(mfxBitstream__bindgen_ty_1__bindgen_ty_1, reserved1) - 18usize];
    ["Offset of field: mfxBitstream__bindgen_ty_1__bindgen_ty_1::CodecId"]
        [::core::mem::offset_of!(mfxBitstream__bindgen_ty_1__bindgen_ty_1, CodecId) - 20usize];
};
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of mfxBitstream__bindgen_ty_1"][::core::mem::size_of::<mfxBitstream__bindgen_ty_1>() - 24usize];
    ["Alignment of mfxBitstream__bindgen_ty_1"][::core::mem::align_of::<mfxBitstream__bindgen_ty_1>() - 8usize];
    ["Offset of field: mfxBitstream__bindgen_ty_1::reserved"]
        [::core::mem::offset_of!(mfxBitstream__bindgen_ty_1, reserved) - 0usize];
};
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of mfxBitstream"][::core::mem::size_of::<mfxBitstream>() - 72usize];
    ["Alignment of mfxBitstream"][::core::mem::align_of::<mfxBitstream>() - 8usize];
    ["Offset of field: mfxBitstream::DecodeTimeStamp"]
        [::core::mem::offset_of!(mfxBitstream, DecodeTimeStamp) - 24usize];
    ["Offset of field: mfxBitstream::TimeStamp"][::core::mem::offset_of!(mfxBitstream, TimeStamp) - 32usize];
    ["Offset of field: mfxBitstream::Data"][::core::mem::offset_of!(mfxBitstream, Data) - 40usize];
    ["Offset of field: mfxBitstream::DataOffset"][::core::mem::offset_of!(mfxBitstream, DataOffset) - 48usize];
    ["Offset of field: mfxBitstream::DataLength"][::core::mem::offset_of!(mfxBitstream, DataLength) - 52usize];
    ["Offset of field: mfxBitstream::MaxLength"][::core::mem::offset_of!(mfxBitstream, MaxLength) - 56usize];
    ["Offset of field: mfxBitstream::PicStruct"][::core::mem::offset_of!(mfxBitstream, PicStruct) - 60usize];
    ["Offset of field: mfxBitstream::FrameType"][::core::mem::offset_of!(mfxBitstream, FrameType) - 62usize];
    ["Offset of field: mfxBitstream::DataFlag"][::core::mem::offset_of!(mfxBitstream, DataFlag) - 64usize];
    ["Offset of field: mfxBitstream::reserved2"][::core::mem::offset_of!(mfxBitstream, reserved2) - 66usize];
};
#[repr(C)]
#[derive(Debug)]
pub struct _mfxSyncPoint {
    _unused: [u8; 0],
}
pub type mfxSyncPoint = *mut _mfxSyncPoint;
pub const MFX_GPUCOPY_DEFAULT: _bindgen_ty_2 = 0;
pub const MFX_GPUCOPY_ON: _bindgen_ty_2 = 1;
pub const MFX_GPUCOPY_OFF: _bindgen_ty_2 = 2;
pub const MFX_GPUCOPY_SAFE: _bindgen_ty_2 = 3;
pub type _bindgen_ty_2 = ::core::ffi::c_int;
pub const MFX_EXTBUFF_THREADS_PARAM: _bindgen_ty_3 = 1346652244;
pub type _bindgen_ty_3 = ::core::ffi::c_int;
pub const MFX_PLATFORM_UNKNOWN: _bindgen_ty_4 = 0;
pub const MFX_PLATFORM_SANDYBRIDGE: _bindgen_ty_4 = 1;
pub const MFX_PLATFORM_IVYBRIDGE: _bindgen_ty_4 = 2;
pub const MFX_PLATFORM_HASWELL: _bindgen_ty_4 = 3;
pub const MFX_PLATFORM_BAYTRAIL: _bindgen_ty_4 = 4;
pub const MFX_PLATFORM_BROADWELL: _bindgen_ty_4 = 5;
pub const MFX_PLATFORM_CHERRYTRAIL: _bindgen_ty_4 = 6;
pub const MFX_PLATFORM_SKYLAKE: _bindgen_ty_4 = 7;
pub const MFX_PLATFORM_APOLLOLAKE: _bindgen_ty_4 = 8;
pub const MFX_PLATFORM_KABYLAKE: _bindgen_ty_4 = 9;
pub const MFX_PLATFORM_GEMINILAKE: _bindgen_ty_4 = 10;
pub const MFX_PLATFORM_COFFEELAKE: _bindgen_ty_4 = 11;
pub const MFX_PLATFORM_CANNONLAKE: _bindgen_ty_4 = 20;
pub const MFX_PLATFORM_ICELAKE: _bindgen_ty_4 = 30;
pub const MFX_PLATFORM_JASPERLAKE: _bindgen_ty_4 = 32;
pub const MFX_PLATFORM_ELKHARTLAKE: _bindgen_ty_4 = 33;
pub const MFX_PLATFORM_TIGERLAKE: _bindgen_ty_4 = 40;
pub const MFX_PLATFORM_ROCKETLAKE: _bindgen_ty_4 = 42;
pub const MFX_PLATFORM_ALDERLAKE_S: _bindgen_ty_4 = 43;
pub const MFX_PLATFORM_ALDERLAKE_P: _bindgen_ty_4 = 44;
pub const MFX_PLATFORM_ARCTICSOUND_P: _bindgen_ty_4 = 45;
pub const MFX_PLATFORM_XEHP_SDV: _bindgen_ty_4 = 45;
pub const MFX_PLATFORM_DG2: _bindgen_ty_4 = 46;
pub const MFX_PLATFORM_ATS_M: _bindgen_ty_4 = 46;
pub const MFX_PLATFORM_ALDERLAKE_N: _bindgen_ty_4 = 55;
pub const MFX_PLATFORM_KEEMBAY: _bindgen_ty_4 = 50;
pub const MFX_PLATFORM_METEORLAKE: _bindgen_ty_4 = 51;
pub const MFX_PLATFORM_BATTLEMAGE: _bindgen_ty_4 = 52;
pub const MFX_PLATFORM_LUNARLAKE: _bindgen_ty_4 = 53;
pub const MFX_PLATFORM_ARROWLAKE: _bindgen_ty_4 = 54;
pub const MFX_PLATFORM_MAXIMUM: _bindgen_ty_4 = 65535;
pub type _bindgen_ty_4 = ::core::ffi::c_int;
pub const MFX_RESOURCE_SYSTEM_SURFACE: mfxResourceType = 1;
pub const MFX_RESOURCE_VA_SURFACE_PTR: mfxResourceType = 2;
pub const MFX_RESOURCE_VA_SURFACE: mfxResourceType = 2;
pub const MFX_RESOURCE_VA_BUFFER_PTR: mfxResourceType = 3;
pub const MFX_RESOURCE_VA_BUFFER: mfxResourceType = 3;
pub const MFX_RESOURCE_DX9_SURFACE: mfxResourceType = 4;
pub const MFX_RESOURCE_DX11_TEXTURE: mfxResourceType = 5;
pub const MFX_RESOURCE_DX12_RESOURCE: mfxResourceType = 6;
pub const MFX_RESOURCE_DMA_RESOURCE: mfxResourceType = 7;
pub const MFX_RESOURCE_HDDLUNITE_REMOTE_MEMORY: mfxResourceType = 8;
pub type mfxResourceType = ::core::ffi::c_int;
pub const MFX_IMPL_TYPE_SOFTWARE: mfxImplType = 1;
pub const MFX_IMPL_TYPE_HARDWARE: mfxImplType = 2;
pub type mfxImplType = ::core::ffi::c_int;
pub const MFX_ACCEL_MODE_NA: mfxAccelerationMode = 0;
pub const MFX_ACCEL_MODE_VIA_D3D9: mfxAccelerationMode = 512;
pub const MFX_ACCEL_MODE_VIA_D3D11: mfxAccelerationMode = 768;
pub const MFX_ACCEL_MODE_VIA_VAAPI: mfxAccelerationMode = 1024;
pub const MFX_ACCEL_MODE_VIA_VAAPI_DRM_RENDER_NODE: mfxAccelerationMode = 1024;
pub const MFX_ACCEL_MODE_VIA_VAAPI_DRM_MODESET: mfxAccelerationMode = 1025;
pub const MFX_ACCEL_MODE_VIA_VAAPI_GLX: mfxAccelerationMode = 1026;
pub const MFX_ACCEL_MODE_VIA_VAAPI_X11: mfxAccelerationMode = 1027;
pub const MFX_ACCEL_MODE_VIA_VAAPI_WAYLAND: mfxAccelerationMode = 1028;
pub const MFX_ACCEL_MODE_VIA_HDDLUNITE: mfxAccelerationMode = 1280;
pub type mfxAccelerationMode = ::core::ffi::c_int;
#[repr(C)]
#[derive(Copy, Clone)]
pub struct mfxExtendedDeviceId {
    pub Version: mfxStructVersion,
    pub VendorID: mfxU16,
    pub DeviceID: mfxU16,
    pub PCIDomain: mfxU32,
    pub PCIBus: mfxU32,
    pub PCIDevice: mfxU32,
    pub PCIFunction: mfxU32,
    pub DeviceLUID: [mfxU8; 8usize],
    pub LUIDDeviceNodeMask: mfxU32,
    pub LUIDValid: mfxU32,
    pub DRMRenderNodeNum: mfxU32,
    pub DRMPrimaryNodeNum: mfxU32,
    pub RevisionID: mfxU16,
    pub reserved1: [mfxU8; 18usize],
    pub DeviceName: [mfxChar; 128usize],
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of mfxExtendedDeviceId"][::core::mem::size_of::<mfxExtendedDeviceId>() - 196usize];
    ["Alignment of mfxExtendedDeviceId"][::core::mem::align_of::<mfxExtendedDeviceId>() - 4usize];
    ["Offset of field: mfxExtendedDeviceId::Version"][::core::mem::offset_of!(mfxExtendedDeviceId, Version) - 0usize];
    ["Offset of field: mfxExtendedDeviceId::VendorID"][::core::mem::offset_of!(mfxExtendedDeviceId, VendorID) - 2usize];
    ["Offset of field: mfxExtendedDeviceId::DeviceID"][::core::mem::offset_of!(mfxExtendedDeviceId, DeviceID) - 4usize];
    ["Offset of field: mfxExtendedDeviceId::PCIDomain"]
        [::core::mem::offset_of!(mfxExtendedDeviceId, PCIDomain) - 8usize];
    ["Offset of field: mfxExtendedDeviceId::PCIBus"][::core::mem::offset_of!(mfxExtendedDeviceId, PCIBus) - 12usize];
    ["Offset of field: mfxExtendedDeviceId::PCIDevice"]
        [::core::mem::offset_of!(mfxExtendedDeviceId, PCIDevice) - 16usize];
    ["Offset of field: mfxExtendedDeviceId::PCIFunction"]
        [::core::mem::offset_of!(mfxExtendedDeviceId, PCIFunction) - 20usize];
    ["Offset of field: mfxExtendedDeviceId::DeviceLUID"]
        [::core::mem::offset_of!(mfxExtendedDeviceId, DeviceLUID) - 24usize];
    ["Offset of field: mfxExtendedDeviceId::LUIDDeviceNodeMask"]
        [::core::mem::offset_of!(mfxExtendedDeviceId, LUIDDeviceNodeMask) - 32usize];
    ["Offset of field: mfxExtendedDeviceId::LUIDValid"]
        [::core::mem::offset_of!(mfxExtendedDeviceId, LUIDValid) - 36usize];
    ["Offset of field: mfxExtendedDeviceId::DRMRenderNodeNum"]
        [::core::mem::offset_of!(mfxExtendedDeviceId, DRMRenderNodeNum) - 40usize];
    ["Offset of field: mfxExtendedDeviceId::DRMPrimaryNodeNum"]
        [::core::mem::offset_of!(mfxExtendedDeviceId, DRMPrimaryNodeNum) - 44usize];
    ["Offset of field: mfxExtendedDeviceId::RevisionID"]
        [::core::mem::offset_of!(mfxExtendedDeviceId, RevisionID) - 48usize];
    ["Offset of field: mfxExtendedDeviceId::reserved1"]
        [::core::mem::offset_of!(mfxExtendedDeviceId, reserved1) - 50usize];
    ["Offset of field: mfxExtendedDeviceId::DeviceName"]
        [::core::mem::offset_of!(mfxExtendedDeviceId, DeviceName) - 68usize];
};
pub const MFX_IMPLCAPS_IMPLDESCSTRUCTURE: mfxImplCapsDeliveryFormat = 1;
pub const MFX_IMPLCAPS_IMPLEMENTEDFUNCTIONS: mfxImplCapsDeliveryFormat = 2;
pub const MFX_IMPLCAPS_IMPLPATH: mfxImplCapsDeliveryFormat = 3;
pub const MFX_IMPLCAPS_DEVICE_ID_EXTENDED: mfxImplCapsDeliveryFormat = 4;
pub type mfxImplCapsDeliveryFormat = ::core::ffi::c_int;
#[repr(C)]
#[derive(Debug)]
pub struct _mfxSession {
    _unused: [u8; 0],
}
pub type mfxSession = *mut _mfxSession;
#[repr(C)]
#[derive(Copy, Clone)]
pub struct mfxFrameId {
    pub TemporalId: mfxU16,
    pub PriorityId: mfxU16,
    pub __bindgen_anon_1: mfxFrameId__bindgen_ty_1,
}
#[repr(C)]
#[derive(Copy, Clone)]
pub union mfxFrameId__bindgen_ty_1 {
    pub __bindgen_anon_1: mfxFrameId__bindgen_ty_1__bindgen_ty_1,
    pub __bindgen_anon_2: mfxFrameId__bindgen_ty_1__bindgen_ty_2,
}
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct mfxFrameId__bindgen_ty_1__bindgen_ty_1 {
    pub DependencyId: mfxU16,
    pub QualityId: mfxU16,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of mfxFrameId__bindgen_ty_1__bindgen_ty_1"]
        [::core::mem::size_of::<mfxFrameId__bindgen_ty_1__bindgen_ty_1>() - 4usize];
    ["Alignment of mfxFrameId__bindgen_ty_1__bindgen_ty_1"]
        [::core::mem::align_of::<mfxFrameId__bindgen_ty_1__bindgen_ty_1>() - 2usize];
    ["Offset of field: mfxFrameId__bindgen_ty_1__bindgen_ty_1::DependencyId"]
        [::core::mem::offset_of!(mfxFrameId__bindgen_ty_1__bindgen_ty_1, DependencyId) - 0usize];
    ["Offset of field: mfxFrameId__bindgen_ty_1__bindgen_ty_1::QualityId"]
        [::core::mem::offset_of!(mfxFrameId__bindgen_ty_1__bindgen_ty_1, QualityId) - 2usize];
};
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct mfxFrameId__bindgen_ty_1__bindgen_ty_2 {
    pub ViewId: mfxU16,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of mfxFrameId__bindgen_ty_1__bindgen_ty_2"]
        [::core::mem::size_of::<mfxFrameId__bindgen_ty_1__bindgen_ty_2>() - 2usize];
    ["Alignment of mfxFrameId__bindgen_ty_1__bindgen_ty_2"]
        [::core::mem::align_of::<mfxFrameId__bindgen_ty_1__bindgen_ty_2>() - 2usize];
    ["Offset of field: mfxFrameId__bindgen_ty_1__bindgen_ty_2::ViewId"]
        [::core::mem::offset_of!(mfxFrameId__bindgen_ty_1__bindgen_ty_2, ViewId) - 0usize];
};
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of mfxFrameId__bindgen_ty_1"][::core::mem::size_of::<mfxFrameId__bindgen_ty_1>() - 4usize];
    ["Alignment of mfxFrameId__bindgen_ty_1"][::core::mem::align_of::<mfxFrameId__bindgen_ty_1>() - 2usize];
};
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of mfxFrameId"][::core::mem::size_of::<mfxFrameId>() - 8usize];
    ["Alignment of mfxFrameId"][::core::mem::align_of::<mfxFrameId>() - 2usize];
    ["Offset of field: mfxFrameId::TemporalId"][::core::mem::offset_of!(mfxFrameId, TemporalId) - 0usize];
    ["Offset of field: mfxFrameId::PriorityId"][::core::mem::offset_of!(mfxFrameId, PriorityId) - 2usize];
};
#[repr(C)]
#[derive(Copy, Clone)]
pub struct mfxFrameInfo {
    pub reserved: [mfxU32; 4usize],
    pub ChannelId: mfxU16,
    pub BitDepthLuma: mfxU16,
    pub BitDepthChroma: mfxU16,
    pub Shift: mfxU16,
    pub FrameId: mfxFrameId,
    pub FourCC: mfxU32,
    pub __bindgen_anon_1: mfxFrameInfo__bindgen_ty_1,
    pub FrameRateExtN: mfxU32,
    pub FrameRateExtD: mfxU32,
    pub reserved3: mfxU16,
    pub AspectRatioW: mfxU16,
    pub AspectRatioH: mfxU16,
    pub PicStruct: mfxU16,
    pub ChromaFormat: mfxU16,
    pub reserved2: mfxU16,
}
#[repr(C)]
#[derive(Copy, Clone)]
pub union mfxFrameInfo__bindgen_ty_1 {
    pub __bindgen_anon_1: mfxFrameInfo__bindgen_ty_1__bindgen_ty_1,
    pub __bindgen_anon_2: mfxFrameInfo__bindgen_ty_1__bindgen_ty_2,
}
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct mfxFrameInfo__bindgen_ty_1__bindgen_ty_1 {
    pub Width: mfxU16,
    pub Height: mfxU16,
    pub CropX: mfxU16,
    pub CropY: mfxU16,
    pub CropW: mfxU16,
    pub CropH: mfxU16,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of mfxFrameInfo__bindgen_ty_1__bindgen_ty_1"]
        [::core::mem::size_of::<mfxFrameInfo__bindgen_ty_1__bindgen_ty_1>() - 12usize];
    ["Alignment of mfxFrameInfo__bindgen_ty_1__bindgen_ty_1"]
        [::core::mem::align_of::<mfxFrameInfo__bindgen_ty_1__bindgen_ty_1>() - 2usize];
    ["Offset of field: mfxFrameInfo__bindgen_ty_1__bindgen_ty_1::Width"]
        [::core::mem::offset_of!(mfxFrameInfo__bindgen_ty_1__bindgen_ty_1, Width) - 0usize];
    ["Offset of field: mfxFrameInfo__bindgen_ty_1__bindgen_ty_1::Height"]
        [::core::mem::offset_of!(mfxFrameInfo__bindgen_ty_1__bindgen_ty_1, Height) - 2usize];
    ["Offset of field: mfxFrameInfo__bindgen_ty_1__bindgen_ty_1::CropX"]
        [::core::mem::offset_of!(mfxFrameInfo__bindgen_ty_1__bindgen_ty_1, CropX) - 4usize];
    ["Offset of field: mfxFrameInfo__bindgen_ty_1__bindgen_ty_1::CropY"]
        [::core::mem::offset_of!(mfxFrameInfo__bindgen_ty_1__bindgen_ty_1, CropY) - 6usize];
    ["Offset of field: mfxFrameInfo__bindgen_ty_1__bindgen_ty_1::CropW"]
        [::core::mem::offset_of!(mfxFrameInfo__bindgen_ty_1__bindgen_ty_1, CropW) - 8usize];
    ["Offset of field: mfxFrameInfo__bindgen_ty_1__bindgen_ty_1::CropH"]
        [::core::mem::offset_of!(mfxFrameInfo__bindgen_ty_1__bindgen_ty_1, CropH) - 10usize];
};
#[repr(C, packed(4))]
#[derive(Debug, Copy, Clone)]
pub struct mfxFrameInfo__bindgen_ty_1__bindgen_ty_2 {
    pub BufferSize: mfxU64,
    pub reserved5: mfxU32,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of mfxFrameInfo__bindgen_ty_1__bindgen_ty_2"]
        [::core::mem::size_of::<mfxFrameInfo__bindgen_ty_1__bindgen_ty_2>() - 12usize];
    ["Alignment of mfxFrameInfo__bindgen_ty_1__bindgen_ty_2"]
        [::core::mem::align_of::<mfxFrameInfo__bindgen_ty_1__bindgen_ty_2>() - 4usize];
    ["Offset of field: mfxFrameInfo__bindgen_ty_1__bindgen_ty_2::BufferSize"]
        [::core::mem::offset_of!(mfxFrameInfo__bindgen_ty_1__bindgen_ty_2, BufferSize) - 0usize];
    ["Offset of field: mfxFrameInfo__bindgen_ty_1__bindgen_ty_2::reserved5"]
        [::core::mem::offset_of!(mfxFrameInfo__bindgen_ty_1__bindgen_ty_2, reserved5) - 8usize];
};
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of mfxFrameInfo__bindgen_ty_1"][::core::mem::size_of::<mfxFrameInfo__bindgen_ty_1>() - 12usize];
    ["Alignment of mfxFrameInfo__bindgen_ty_1"][::core::mem::align_of::<mfxFrameInfo__bindgen_ty_1>() - 4usize];
};
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of mfxFrameInfo"][::core::mem::size_of::<mfxFrameInfo>() - 68usize];
    ["Alignment of mfxFrameInfo"][::core::mem::align_of::<mfxFrameInfo>() - 4usize];
    ["Offset of field: mfxFrameInfo::reserved"][::core::mem::offset_of!(mfxFrameInfo, reserved) - 0usize];
    ["Offset of field: mfxFrameInfo::ChannelId"][::core::mem::offset_of!(mfxFrameInfo, ChannelId) - 16usize];
    ["Offset of field: mfxFrameInfo::BitDepthLuma"][::core::mem::offset_of!(mfxFrameInfo, BitDepthLuma) - 18usize];
    ["Offset of field: mfxFrameInfo::BitDepthChroma"][::core::mem::offset_of!(mfxFrameInfo, BitDepthChroma) - 20usize];
    ["Offset of field: mfxFrameInfo::Shift"][::core::mem::offset_of!(mfxFrameInfo, Shift) - 22usize];
    ["Offset of field: mfxFrameInfo::FrameId"][::core::mem::offset_of!(mfxFrameInfo, FrameId) - 24usize];
    ["Offset of field: mfxFrameInfo::FourCC"][::core::mem::offset_of!(mfxFrameInfo, FourCC) - 32usize];
    ["Offset of field: mfxFrameInfo::FrameRateExtN"][::core::mem::offset_of!(mfxFrameInfo, FrameRateExtN) - 48usize];
    ["Offset of field: mfxFrameInfo::FrameRateExtD"][::core::mem::offset_of!(mfxFrameInfo, FrameRateExtD) - 52usize];
    ["Offset of field: mfxFrameInfo::reserved3"][::core::mem::offset_of!(mfxFrameInfo, reserved3) - 56usize];
    ["Offset of field: mfxFrameInfo::AspectRatioW"][::core::mem::offset_of!(mfxFrameInfo, AspectRatioW) - 58usize];
    ["Offset of field: mfxFrameInfo::AspectRatioH"][::core::mem::offset_of!(mfxFrameInfo, AspectRatioH) - 60usize];
    ["Offset of field: mfxFrameInfo::PicStruct"][::core::mem::offset_of!(mfxFrameInfo, PicStruct) - 62usize];
    ["Offset of field: mfxFrameInfo::ChromaFormat"][::core::mem::offset_of!(mfxFrameInfo, ChromaFormat) - 64usize];
    ["Offset of field: mfxFrameInfo::reserved2"][::core::mem::offset_of!(mfxFrameInfo, reserved2) - 66usize];
};
pub const MFX_FOURCC_NV12: _bindgen_ty_5 = 842094158;
pub const MFX_FOURCC_YV12: _bindgen_ty_5 = 842094169;
pub const MFX_FOURCC_NV16: _bindgen_ty_5 = 909203022;
pub const MFX_FOURCC_YUY2: _bindgen_ty_5 = 844715353;
pub const MFX_FOURCC_RGB565: _bindgen_ty_5 = 843204434;
pub const MFX_FOURCC_RGBP: _bindgen_ty_5 = 1346520914;
pub const MFX_FOURCC_RGB3: _bindgen_ty_5 = 859981650;
pub const MFX_FOURCC_RGB4: _bindgen_ty_5 = 876758866;
pub const MFX_FOURCC_P8: _bindgen_ty_5 = 41;
pub const MFX_FOURCC_P8_TEXTURE: _bindgen_ty_5 = 1112356944;
pub const MFX_FOURCC_P010: _bindgen_ty_5 = 808530000;
pub const MFX_FOURCC_P016: _bindgen_ty_5 = 909193296;
pub const MFX_FOURCC_P210: _bindgen_ty_5 = 808530512;
pub const MFX_FOURCC_BGR4: _bindgen_ty_5 = 877807426;
pub const MFX_FOURCC_A2RGB10: _bindgen_ty_5 = 808535890;
pub const MFX_FOURCC_ARGB16: _bindgen_ty_5 = 909199186;
pub const MFX_FOURCC_ABGR16: _bindgen_ty_5 = 909199170;
pub const MFX_FOURCC_R16: _bindgen_ty_5 = 1429614930;
pub const MFX_FOURCC_AYUV: _bindgen_ty_5 = 1448433985;
pub const MFX_FOURCC_AYUV_RGB4: _bindgen_ty_5 = 1498764865;
pub const MFX_FOURCC_UYVY: _bindgen_ty_5 = 1498831189;
pub const MFX_FOURCC_Y210: _bindgen_ty_5 = 808530521;
pub const MFX_FOURCC_Y410: _bindgen_ty_5 = 808531033;
pub const MFX_FOURCC_Y216: _bindgen_ty_5 = 909193817;
pub const MFX_FOURCC_Y416: _bindgen_ty_5 = 909194329;
pub const MFX_FOURCC_NV21: _bindgen_ty_5 = 825382478;
pub const MFX_FOURCC_IYUV: _bindgen_ty_5 = 1448433993;
pub const MFX_FOURCC_I010: _bindgen_ty_5 = 808529993;
pub const MFX_FOURCC_I210: _bindgen_ty_5 = 808530505;
pub const MFX_FOURCC_I420: _bindgen_ty_5 = 1448433993;
pub const MFX_FOURCC_I422: _bindgen_ty_5 = 842150985;
pub const MFX_FOURCC_BGRA: _bindgen_ty_5 = 876758866;
pub const MFX_FOURCC_BGRP: _bindgen_ty_5 = 1347569474;
pub const MFX_FOURCC_XYUV: _bindgen_ty_5 = 1448434008;
pub const MFX_FOURCC_ABGR16F: _bindgen_ty_5 = 1179797314;
pub type _bindgen_ty_5 = ::core::ffi::c_int;
pub const MFX_PICSTRUCT_UNKNOWN: _bindgen_ty_6 = 0;
pub const MFX_PICSTRUCT_PROGRESSIVE: _bindgen_ty_6 = 1;
pub const MFX_PICSTRUCT_FIELD_TFF: _bindgen_ty_6 = 2;
pub const MFX_PICSTRUCT_FIELD_BFF: _bindgen_ty_6 = 4;
pub const MFX_PICSTRUCT_FIELD_REPEATED: _bindgen_ty_6 = 16;
pub const MFX_PICSTRUCT_FRAME_DOUBLING: _bindgen_ty_6 = 32;
pub const MFX_PICSTRUCT_FRAME_TRIPLING: _bindgen_ty_6 = 64;
pub const MFX_PICSTRUCT_FIELD_SINGLE: _bindgen_ty_6 = 256;
pub const MFX_PICSTRUCT_FIELD_TOP: _bindgen_ty_6 = 258;
pub const MFX_PICSTRUCT_FIELD_BOTTOM: _bindgen_ty_6 = 260;
pub const MFX_PICSTRUCT_FIELD_PAIRED_PREV: _bindgen_ty_6 = 512;
pub const MFX_PICSTRUCT_FIELD_PAIRED_NEXT: _bindgen_ty_6 = 1024;
pub type _bindgen_ty_6 = ::core::ffi::c_int;
pub const MFX_CHROMAFORMAT_MONOCHROME: _bindgen_ty_7 = 0;
pub const MFX_CHROMAFORMAT_YUV420: _bindgen_ty_7 = 1;
pub const MFX_CHROMAFORMAT_YUV422: _bindgen_ty_7 = 2;
pub const MFX_CHROMAFORMAT_YUV444: _bindgen_ty_7 = 3;
pub const MFX_CHROMAFORMAT_YUV400: _bindgen_ty_7 = 0;
pub const MFX_CHROMAFORMAT_YUV411: _bindgen_ty_7 = 4;
pub const MFX_CHROMAFORMAT_YUV422H: _bindgen_ty_7 = 2;
pub const MFX_CHROMAFORMAT_YUV422V: _bindgen_ty_7 = 5;
pub const MFX_CHROMAFORMAT_RESERVED1: _bindgen_ty_7 = 6;
pub type _bindgen_ty_7 = ::core::ffi::c_int;
pub const MFX_TIMESTAMP_UNKNOWN: _bindgen_ty_8 = -1;
pub type _bindgen_ty_8 = ::core::ffi::c_int;
pub const MFX_FRAMEORDER_UNKNOWN: _bindgen_ty_9 = -1;
pub type _bindgen_ty_9 = ::core::ffi::c_int;
pub const MFX_FRAMEDATA_TIMESTAMP_UNKNOWN: _bindgen_ty_10 = 0;
pub const MFX_FRAMEDATA_ORIGINAL_TIMESTAMP: _bindgen_ty_10 = 1;
pub type _bindgen_ty_10 = ::core::ffi::c_int;
pub const MFX_CORRUPTION_NO: _bindgen_ty_11 = 0;
pub const MFX_CORRUPTION_MINOR: _bindgen_ty_11 = 1;
pub const MFX_CORRUPTION_MAJOR: _bindgen_ty_11 = 2;
pub const MFX_CORRUPTION_ABSENT_TOP_FIELD: _bindgen_ty_11 = 4;
pub const MFX_CORRUPTION_ABSENT_BOTTOM_FIELD: _bindgen_ty_11 = 8;
pub const MFX_CORRUPTION_REFERENCE_FRAME: _bindgen_ty_11 = 16;
pub const MFX_CORRUPTION_REFERENCE_LIST: _bindgen_ty_11 = 32;
pub type _bindgen_ty_11 = ::core::ffi::c_int;
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct mfxY410 {
    pub _bindgen_align: [u32; 0],
    pub _bitfield_1: __BindgenBitfieldUnit<[u8; 4usize]>,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of mfxY410"][::core::mem::size_of::<mfxY410>() - 4usize];
    ["Alignment of mfxY410"][::core::mem::align_of::<mfxY410>() - 4usize];
};
impl mfxY410 {
    #[inline]
    #[allow(unnecessary_transmutes)]
    pub fn U(&self) -> mfxU32 {
        unsafe { ::core::mem::transmute(self._bitfield_1.get_const::<0usize, 10u8>() as u32) }
    }
    #[inline]
    #[allow(unnecessary_transmutes)]
    pub fn set_U(&mut self, val: mfxU32) {
        unsafe {
            let val: u32 = ::core::mem::transmute(val);
            self._bitfield_1.set_const::<0usize, 10u8>(val as u64)
        }
    }
    #[inline]
    #[allow(unnecessary_transmutes)]
    pub unsafe fn U_raw(this: *const Self) -> mfxU32 {
        unsafe {
            ::core::mem::transmute(<__BindgenBitfieldUnit<[u8; 4usize]>>::raw_get_const::<0usize, 10u8>(
                ::core::ptr::addr_of!((*this)._bitfield_1),
            ) as u32)
        }
    }
    #[inline]
    #[allow(unnecessary_transmutes)]
    pub unsafe fn set_U_raw(this: *mut Self, val: mfxU32) {
        unsafe {
            let val: u32 = ::core::mem::transmute(val);
            <__BindgenBitfieldUnit<[u8; 4usize]>>::raw_set_const::<0usize, 10u8>(
                ::core::ptr::addr_of_mut!((*this)._bitfield_1),
                val as u64,
            )
        }
    }
    #[inline]
    #[allow(unnecessary_transmutes)]
    pub fn Y(&self) -> mfxU32 {
        unsafe { ::core::mem::transmute(self._bitfield_1.get_const::<10usize, 10u8>() as u32) }
    }
    #[inline]
    #[allow(unnecessary_transmutes)]
    pub fn set_Y(&mut self, val: mfxU32) {
        unsafe {
            let val: u32 = ::core::mem::transmute(val);
            self._bitfield_1.set_const::<10usize, 10u8>(val as u64)
        }
    }
    #[inline]
    #[allow(unnecessary_transmutes)]
    pub unsafe fn Y_raw(this: *const Self) -> mfxU32 {
        unsafe {
            ::core::mem::transmute(<__BindgenBitfieldUnit<[u8; 4usize]>>::raw_get_const::<10usize, 10u8>(
                ::core::ptr::addr_of!((*this)._bitfield_1),
            ) as u32)
        }
    }
    #[inline]
    #[allow(unnecessary_transmutes)]
    pub unsafe fn set_Y_raw(this: *mut Self, val: mfxU32) {
        unsafe {
            let val: u32 = ::core::mem::transmute(val);
            <__BindgenBitfieldUnit<[u8; 4usize]>>::raw_set_const::<10usize, 10u8>(
                ::core::ptr::addr_of_mut!((*this)._bitfield_1),
                val as u64,
            )
        }
    }
    #[inline]
    #[allow(unnecessary_transmutes)]
    pub fn V(&self) -> mfxU32 {
        unsafe { ::core::mem::transmute(self._bitfield_1.get_const::<20usize, 10u8>() as u32) }
    }
    #[inline]
    #[allow(unnecessary_transmutes)]
    pub fn set_V(&mut self, val: mfxU32) {
        unsafe {
            let val: u32 = ::core::mem::transmute(val);
            self._bitfield_1.set_const::<20usize, 10u8>(val as u64)
        }
    }
    #[inline]
    #[allow(unnecessary_transmutes)]
    pub unsafe fn V_raw(this: *const Self) -> mfxU32 {
        unsafe {
            ::core::mem::transmute(<__BindgenBitfieldUnit<[u8; 4usize]>>::raw_get_const::<20usize, 10u8>(
                ::core::ptr::addr_of!((*this)._bitfield_1),
            ) as u32)
        }
    }
    #[inline]
    #[allow(unnecessary_transmutes)]
    pub unsafe fn set_V_raw(this: *mut Self, val: mfxU32) {
        unsafe {
            let val: u32 = ::core::mem::transmute(val);
            <__BindgenBitfieldUnit<[u8; 4usize]>>::raw_set_const::<20usize, 10u8>(
                ::core::ptr::addr_of_mut!((*this)._bitfield_1),
                val as u64,
            )
        }
    }
    #[inline]
    #[allow(unnecessary_transmutes)]
    pub fn A(&self) -> mfxU32 {
        unsafe { ::core::mem::transmute(self._bitfield_1.get_const::<30usize, 2u8>() as u32) }
    }
    #[inline]
    #[allow(unnecessary_transmutes)]
    pub fn set_A(&mut self, val: mfxU32) {
        unsafe {
            let val: u32 = ::core::mem::transmute(val);
            self._bitfield_1.set_const::<30usize, 2u8>(val as u64)
        }
    }
    #[inline]
    #[allow(unnecessary_transmutes)]
    pub unsafe fn A_raw(this: *const Self) -> mfxU32 {
        unsafe {
            ::core::mem::transmute(<__BindgenBitfieldUnit<[u8; 4usize]>>::raw_get_const::<30usize, 2u8>(
                ::core::ptr::addr_of!((*this)._bitfield_1),
            ) as u32)
        }
    }
    #[inline]
    #[allow(unnecessary_transmutes)]
    pub unsafe fn set_A_raw(this: *mut Self, val: mfxU32) {
        unsafe {
            let val: u32 = ::core::mem::transmute(val);
            <__BindgenBitfieldUnit<[u8; 4usize]>>::raw_set_const::<30usize, 2u8>(
                ::core::ptr::addr_of_mut!((*this)._bitfield_1),
                val as u64,
            )
        }
    }
    #[inline]
    #[allow(unnecessary_transmutes)]
    pub fn new_bitfield_1(U: mfxU32, Y: mfxU32, V: mfxU32, A: mfxU32) -> __BindgenBitfieldUnit<[u8; 4usize]> {
        let mut __bindgen_bitfield_unit: __BindgenBitfieldUnit<[u8; 4usize]> = Default::default();
        __bindgen_bitfield_unit.set_const::<0usize, 10u8>({
            let U: u32 = unsafe { ::core::mem::transmute(U) };
            U as u64
        });
        __bindgen_bitfield_unit.set_const::<10usize, 10u8>({
            let Y: u32 = unsafe { ::core::mem::transmute(Y) };
            Y as u64
        });
        __bindgen_bitfield_unit.set_const::<20usize, 10u8>({
            let V: u32 = unsafe { ::core::mem::transmute(V) };
            V as u64
        });
        __bindgen_bitfield_unit.set_const::<30usize, 2u8>({
            let A: u32 = unsafe { ::core::mem::transmute(A) };
            A as u64
        });
        __bindgen_bitfield_unit
    }
}
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct mfxY416 {
    pub _bindgen_align: [u32; 0],
    pub _bitfield_1: __BindgenBitfieldUnit<[u8; 8usize]>,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of mfxY416"][::core::mem::size_of::<mfxY416>() - 8usize];
    ["Alignment of mfxY416"][::core::mem::align_of::<mfxY416>() - 4usize];
};
impl mfxY416 {
    #[inline]
    #[allow(unnecessary_transmutes)]
    pub fn U(&self) -> mfxU32 {
        unsafe { ::core::mem::transmute(self._bitfield_1.get_const::<0usize, 16u8>() as u32) }
    }
    #[inline]
    #[allow(unnecessary_transmutes)]
    pub fn set_U(&mut self, val: mfxU32) {
        unsafe {
            let val: u32 = ::core::mem::transmute(val);
            self._bitfield_1.set_const::<0usize, 16u8>(val as u64)
        }
    }
    #[inline]
    #[allow(unnecessary_transmutes)]
    pub unsafe fn U_raw(this: *const Self) -> mfxU32 {
        unsafe {
            ::core::mem::transmute(<__BindgenBitfieldUnit<[u8; 8usize]>>::raw_get_const::<0usize, 16u8>(
                ::core::ptr::addr_of!((*this)._bitfield_1),
            ) as u32)
        }
    }
    #[inline]
    #[allow(unnecessary_transmutes)]
    pub unsafe fn set_U_raw(this: *mut Self, val: mfxU32) {
        unsafe {
            let val: u32 = ::core::mem::transmute(val);
            <__BindgenBitfieldUnit<[u8; 8usize]>>::raw_set_const::<0usize, 16u8>(
                ::core::ptr::addr_of_mut!((*this)._bitfield_1),
                val as u64,
            )
        }
    }
    #[inline]
    #[allow(unnecessary_transmutes)]
    pub fn Y(&self) -> mfxU32 {
        unsafe { ::core::mem::transmute(self._bitfield_1.get_const::<16usize, 16u8>() as u32) }
    }
    #[inline]
    #[allow(unnecessary_transmutes)]
    pub fn set_Y(&mut self, val: mfxU32) {
        unsafe {
            let val: u32 = ::core::mem::transmute(val);
            self._bitfield_1.set_const::<16usize, 16u8>(val as u64)
        }
    }
    #[inline]
    #[allow(unnecessary_transmutes)]
    pub unsafe fn Y_raw(this: *const Self) -> mfxU32 {
        unsafe {
            ::core::mem::transmute(<__BindgenBitfieldUnit<[u8; 8usize]>>::raw_get_const::<16usize, 16u8>(
                ::core::ptr::addr_of!((*this)._bitfield_1),
            ) as u32)
        }
    }
    #[inline]
    #[allow(unnecessary_transmutes)]
    pub unsafe fn set_Y_raw(this: *mut Self, val: mfxU32) {
        unsafe {
            let val: u32 = ::core::mem::transmute(val);
            <__BindgenBitfieldUnit<[u8; 8usize]>>::raw_set_const::<16usize, 16u8>(
                ::core::ptr::addr_of_mut!((*this)._bitfield_1),
                val as u64,
            )
        }
    }
    #[inline]
    #[allow(unnecessary_transmutes)]
    pub fn V(&self) -> mfxU32 {
        unsafe { ::core::mem::transmute(self._bitfield_1.get_const::<32usize, 16u8>() as u32) }
    }
    #[inline]
    #[allow(unnecessary_transmutes)]
    pub fn set_V(&mut self, val: mfxU32) {
        unsafe {
            let val: u32 = ::core::mem::transmute(val);
            self._bitfield_1.set_const::<32usize, 16u8>(val as u64)
        }
    }
    #[inline]
    #[allow(unnecessary_transmutes)]
    pub unsafe fn V_raw(this: *const Self) -> mfxU32 {
        unsafe {
            ::core::mem::transmute(<__BindgenBitfieldUnit<[u8; 8usize]>>::raw_get_const::<32usize, 16u8>(
                ::core::ptr::addr_of!((*this)._bitfield_1),
            ) as u32)
        }
    }
    #[inline]
    #[allow(unnecessary_transmutes)]
    pub unsafe fn set_V_raw(this: *mut Self, val: mfxU32) {
        unsafe {
            let val: u32 = ::core::mem::transmute(val);
            <__BindgenBitfieldUnit<[u8; 8usize]>>::raw_set_const::<32usize, 16u8>(
                ::core::ptr::addr_of_mut!((*this)._bitfield_1),
                val as u64,
            )
        }
    }
    #[inline]
    #[allow(unnecessary_transmutes)]
    pub fn A(&self) -> mfxU32 {
        unsafe { ::core::mem::transmute(self._bitfield_1.get_const::<48usize, 16u8>() as u32) }
    }
    #[inline]
    #[allow(unnecessary_transmutes)]
    pub fn set_A(&mut self, val: mfxU32) {
        unsafe {
            let val: u32 = ::core::mem::transmute(val);
            self._bitfield_1.set_const::<48usize, 16u8>(val as u64)
        }
    }
    #[inline]
    #[allow(unnecessary_transmutes)]
    pub unsafe fn A_raw(this: *const Self) -> mfxU32 {
        unsafe {
            ::core::mem::transmute(<__BindgenBitfieldUnit<[u8; 8usize]>>::raw_get_const::<48usize, 16u8>(
                ::core::ptr::addr_of!((*this)._bitfield_1),
            ) as u32)
        }
    }
    #[inline]
    #[allow(unnecessary_transmutes)]
    pub unsafe fn set_A_raw(this: *mut Self, val: mfxU32) {
        unsafe {
            let val: u32 = ::core::mem::transmute(val);
            <__BindgenBitfieldUnit<[u8; 8usize]>>::raw_set_const::<48usize, 16u8>(
                ::core::ptr::addr_of_mut!((*this)._bitfield_1),
                val as u64,
            )
        }
    }
    #[inline]
    #[allow(unnecessary_transmutes)]
    pub fn new_bitfield_1(U: mfxU32, Y: mfxU32, V: mfxU32, A: mfxU32) -> __BindgenBitfieldUnit<[u8; 8usize]> {
        let mut __bindgen_bitfield_unit: __BindgenBitfieldUnit<[u8; 8usize]> = Default::default();
        __bindgen_bitfield_unit.set_const::<0usize, 16u8>({
            let U: u32 = unsafe { ::core::mem::transmute(U) };
            U as u64
        });
        __bindgen_bitfield_unit.set_const::<16usize, 16u8>({
            let Y: u32 = unsafe { ::core::mem::transmute(Y) };
            Y as u64
        });
        __bindgen_bitfield_unit.set_const::<32usize, 16u8>({
            let V: u32 = unsafe { ::core::mem::transmute(V) };
            V as u64
        });
        __bindgen_bitfield_unit.set_const::<48usize, 16u8>({
            let A: u32 = unsafe { ::core::mem::transmute(A) };
            A as u64
        });
        __bindgen_bitfield_unit
    }
}
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct mfxA2RGB10 {
    pub _bindgen_align: [u32; 0],
    pub _bitfield_1: __BindgenBitfieldUnit<[u8; 4usize]>,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of mfxA2RGB10"][::core::mem::size_of::<mfxA2RGB10>() - 4usize];
    ["Alignment of mfxA2RGB10"][::core::mem::align_of::<mfxA2RGB10>() - 4usize];
};
impl mfxA2RGB10 {
    #[inline]
    #[allow(unnecessary_transmutes)]
    pub fn B(&self) -> mfxU32 {
        unsafe { ::core::mem::transmute(self._bitfield_1.get_const::<0usize, 10u8>() as u32) }
    }
    #[inline]
    #[allow(unnecessary_transmutes)]
    pub fn set_B(&mut self, val: mfxU32) {
        unsafe {
            let val: u32 = ::core::mem::transmute(val);
            self._bitfield_1.set_const::<0usize, 10u8>(val as u64)
        }
    }
    #[inline]
    #[allow(unnecessary_transmutes)]
    pub unsafe fn B_raw(this: *const Self) -> mfxU32 {
        unsafe {
            ::core::mem::transmute(<__BindgenBitfieldUnit<[u8; 4usize]>>::raw_get_const::<0usize, 10u8>(
                ::core::ptr::addr_of!((*this)._bitfield_1),
            ) as u32)
        }
    }
    #[inline]
    #[allow(unnecessary_transmutes)]
    pub unsafe fn set_B_raw(this: *mut Self, val: mfxU32) {
        unsafe {
            let val: u32 = ::core::mem::transmute(val);
            <__BindgenBitfieldUnit<[u8; 4usize]>>::raw_set_const::<0usize, 10u8>(
                ::core::ptr::addr_of_mut!((*this)._bitfield_1),
                val as u64,
            )
        }
    }
    #[inline]
    #[allow(unnecessary_transmutes)]
    pub fn G(&self) -> mfxU32 {
        unsafe { ::core::mem::transmute(self._bitfield_1.get_const::<10usize, 10u8>() as u32) }
    }
    #[inline]
    #[allow(unnecessary_transmutes)]
    pub fn set_G(&mut self, val: mfxU32) {
        unsafe {
            let val: u32 = ::core::mem::transmute(val);
            self._bitfield_1.set_const::<10usize, 10u8>(val as u64)
        }
    }
    #[inline]
    #[allow(unnecessary_transmutes)]
    pub unsafe fn G_raw(this: *const Self) -> mfxU32 {
        unsafe {
            ::core::mem::transmute(<__BindgenBitfieldUnit<[u8; 4usize]>>::raw_get_const::<10usize, 10u8>(
                ::core::ptr::addr_of!((*this)._bitfield_1),
            ) as u32)
        }
    }
    #[inline]
    #[allow(unnecessary_transmutes)]
    pub unsafe fn set_G_raw(this: *mut Self, val: mfxU32) {
        unsafe {
            let val: u32 = ::core::mem::transmute(val);
            <__BindgenBitfieldUnit<[u8; 4usize]>>::raw_set_const::<10usize, 10u8>(
                ::core::ptr::addr_of_mut!((*this)._bitfield_1),
                val as u64,
            )
        }
    }
    #[inline]
    #[allow(unnecessary_transmutes)]
    pub fn R(&self) -> mfxU32 {
        unsafe { ::core::mem::transmute(self._bitfield_1.get_const::<20usize, 10u8>() as u32) }
    }
    #[inline]
    #[allow(unnecessary_transmutes)]
    pub fn set_R(&mut self, val: mfxU32) {
        unsafe {
            let val: u32 = ::core::mem::transmute(val);
            self._bitfield_1.set_const::<20usize, 10u8>(val as u64)
        }
    }
    #[inline]
    #[allow(unnecessary_transmutes)]
    pub unsafe fn R_raw(this: *const Self) -> mfxU32 {
        unsafe {
            ::core::mem::transmute(<__BindgenBitfieldUnit<[u8; 4usize]>>::raw_get_const::<20usize, 10u8>(
                ::core::ptr::addr_of!((*this)._bitfield_1),
            ) as u32)
        }
    }
    #[inline]
    #[allow(unnecessary_transmutes)]
    pub unsafe fn set_R_raw(this: *mut Self, val: mfxU32) {
        unsafe {
            let val: u32 = ::core::mem::transmute(val);
            <__BindgenBitfieldUnit<[u8; 4usize]>>::raw_set_const::<20usize, 10u8>(
                ::core::ptr::addr_of_mut!((*this)._bitfield_1),
                val as u64,
            )
        }
    }
    #[inline]
    #[allow(unnecessary_transmutes)]
    pub fn A(&self) -> mfxU32 {
        unsafe { ::core::mem::transmute(self._bitfield_1.get_const::<30usize, 2u8>() as u32) }
    }
    #[inline]
    #[allow(unnecessary_transmutes)]
    pub fn set_A(&mut self, val: mfxU32) {
        unsafe {
            let val: u32 = ::core::mem::transmute(val);
            self._bitfield_1.set_const::<30usize, 2u8>(val as u64)
        }
    }
    #[inline]
    #[allow(unnecessary_transmutes)]
    pub unsafe fn A_raw(this: *const Self) -> mfxU32 {
        unsafe {
            ::core::mem::transmute(<__BindgenBitfieldUnit<[u8; 4usize]>>::raw_get_const::<30usize, 2u8>(
                ::core::ptr::addr_of!((*this)._bitfield_1),
            ) as u32)
        }
    }
    #[inline]
    #[allow(unnecessary_transmutes)]
    pub unsafe fn set_A_raw(this: *mut Self, val: mfxU32) {
        unsafe {
            let val: u32 = ::core::mem::transmute(val);
            <__BindgenBitfieldUnit<[u8; 4usize]>>::raw_set_const::<30usize, 2u8>(
                ::core::ptr::addr_of_mut!((*this)._bitfield_1),
                val as u64,
            )
        }
    }
    #[inline]
    #[allow(unnecessary_transmutes)]
    pub fn new_bitfield_1(B: mfxU32, G: mfxU32, R: mfxU32, A: mfxU32) -> __BindgenBitfieldUnit<[u8; 4usize]> {
        let mut __bindgen_bitfield_unit: __BindgenBitfieldUnit<[u8; 4usize]> = Default::default();
        __bindgen_bitfield_unit.set_const::<0usize, 10u8>({
            let B: u32 = unsafe { ::core::mem::transmute(B) };
            B as u64
        });
        __bindgen_bitfield_unit.set_const::<10usize, 10u8>({
            let G: u32 = unsafe { ::core::mem::transmute(G) };
            G as u64
        });
        __bindgen_bitfield_unit.set_const::<20usize, 10u8>({
            let R: u32 = unsafe { ::core::mem::transmute(R) };
            R as u64
        });
        __bindgen_bitfield_unit.set_const::<30usize, 2u8>({
            let A: u32 = unsafe { ::core::mem::transmute(A) };
            A as u64
        });
        __bindgen_bitfield_unit
    }
}
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct mfxABGR16FP {
    pub R: mfxFP16,
    pub G: mfxFP16,
    pub B: mfxFP16,
    pub A: mfxFP16,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of mfxABGR16FP"][::core::mem::size_of::<mfxABGR16FP>() - 8usize];
    ["Alignment of mfxABGR16FP"][::core::mem::align_of::<mfxABGR16FP>() - 2usize];
    ["Offset of field: mfxABGR16FP::R"][::core::mem::offset_of!(mfxABGR16FP, R) - 0usize];
    ["Offset of field: mfxABGR16FP::G"][::core::mem::offset_of!(mfxABGR16FP, G) - 2usize];
    ["Offset of field: mfxABGR16FP::B"][::core::mem::offset_of!(mfxABGR16FP, B) - 4usize];
    ["Offset of field: mfxABGR16FP::A"][::core::mem::offset_of!(mfxABGR16FP, A) - 6usize];
};
#[repr(C)]
#[derive(Copy, Clone)]
pub struct mfxFrameData {
    pub __bindgen_anon_1: mfxFrameData__bindgen_ty_1,
    pub NumExtParam: mfxU16,
    pub reserved: [mfxU16; 9usize],
    pub MemType: mfxU16,
    pub PitchHigh: mfxU16,
    pub TimeStamp: mfxU64,
    pub FrameOrder: mfxU32,
    pub Locked: mfxU16,
    pub __bindgen_anon_2: mfxFrameData__bindgen_ty_2,
    pub __bindgen_anon_3: mfxFrameData__bindgen_ty_3,
    pub __bindgen_anon_4: mfxFrameData__bindgen_ty_4,
    pub __bindgen_anon_5: mfxFrameData__bindgen_ty_5,
    pub A: *mut mfxU8,
    pub MemId: mfxMemId,
    pub Corrupted: mfxU16,
    pub DataFlag: mfxU16,
}
#[repr(C)]
#[derive(Copy, Clone)]
pub union mfxFrameData__bindgen_ty_1 {
    pub ExtParam: *mut *mut mfxExtBuffer,
    pub reserved2: mfxU64,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of mfxFrameData__bindgen_ty_1"][::core::mem::size_of::<mfxFrameData__bindgen_ty_1>() - 8usize];
    ["Alignment of mfxFrameData__bindgen_ty_1"][::core::mem::align_of::<mfxFrameData__bindgen_ty_1>() - 8usize];
    ["Offset of field: mfxFrameData__bindgen_ty_1::ExtParam"]
        [::core::mem::offset_of!(mfxFrameData__bindgen_ty_1, ExtParam) - 0usize];
    ["Offset of field: mfxFrameData__bindgen_ty_1::reserved2"]
        [::core::mem::offset_of!(mfxFrameData__bindgen_ty_1, reserved2) - 0usize];
};
#[repr(C)]
#[derive(Copy, Clone)]
pub union mfxFrameData__bindgen_ty_2 {
    pub Pitch: mfxU16,
    pub PitchLow: mfxU16,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of mfxFrameData__bindgen_ty_2"][::core::mem::size_of::<mfxFrameData__bindgen_ty_2>() - 2usize];
    ["Alignment of mfxFrameData__bindgen_ty_2"][::core::mem::align_of::<mfxFrameData__bindgen_ty_2>() - 2usize];
    ["Offset of field: mfxFrameData__bindgen_ty_2::Pitch"]
        [::core::mem::offset_of!(mfxFrameData__bindgen_ty_2, Pitch) - 0usize];
    ["Offset of field: mfxFrameData__bindgen_ty_2::PitchLow"]
        [::core::mem::offset_of!(mfxFrameData__bindgen_ty_2, PitchLow) - 0usize];
};
#[repr(C)]
#[derive(Copy, Clone)]
pub union mfxFrameData__bindgen_ty_3 {
    pub Y: *mut mfxU8,
    pub Y16: *mut mfxU16,
    pub R: *mut mfxU8,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of mfxFrameData__bindgen_ty_3"][::core::mem::size_of::<mfxFrameData__bindgen_ty_3>() - 8usize];
    ["Alignment of mfxFrameData__bindgen_ty_3"][::core::mem::align_of::<mfxFrameData__bindgen_ty_3>() - 8usize];
    ["Offset of field: mfxFrameData__bindgen_ty_3::Y"][::core::mem::offset_of!(mfxFrameData__bindgen_ty_3, Y) - 0usize];
    ["Offset of field: mfxFrameData__bindgen_ty_3::Y16"]
        [::core::mem::offset_of!(mfxFrameData__bindgen_ty_3, Y16) - 0usize];
    ["Offset of field: mfxFrameData__bindgen_ty_3::R"][::core::mem::offset_of!(mfxFrameData__bindgen_ty_3, R) - 0usize];
};
#[repr(C)]
#[derive(Copy, Clone)]
pub union mfxFrameData__bindgen_ty_4 {
    pub UV: *mut mfxU8,
    pub VU: *mut mfxU8,
    pub CbCr: *mut mfxU8,
    pub CrCb: *mut mfxU8,
    pub Cb: *mut mfxU8,
    pub U: *mut mfxU8,
    pub U16: *mut mfxU16,
    pub G: *mut mfxU8,
    pub Y410: *mut mfxY410,
    pub Y416: *mut mfxY416,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of mfxFrameData__bindgen_ty_4"][::core::mem::size_of::<mfxFrameData__bindgen_ty_4>() - 8usize];
    ["Alignment of mfxFrameData__bindgen_ty_4"][::core::mem::align_of::<mfxFrameData__bindgen_ty_4>() - 8usize];
    ["Offset of field: mfxFrameData__bindgen_ty_4::UV"]
        [::core::mem::offset_of!(mfxFrameData__bindgen_ty_4, UV) - 0usize];
    ["Offset of field: mfxFrameData__bindgen_ty_4::VU"]
        [::core::mem::offset_of!(mfxFrameData__bindgen_ty_4, VU) - 0usize];
    ["Offset of field: mfxFrameData__bindgen_ty_4::CbCr"]
        [::core::mem::offset_of!(mfxFrameData__bindgen_ty_4, CbCr) - 0usize];
    ["Offset of field: mfxFrameData__bindgen_ty_4::CrCb"]
        [::core::mem::offset_of!(mfxFrameData__bindgen_ty_4, CrCb) - 0usize];
    ["Offset of field: mfxFrameData__bindgen_ty_4::Cb"]
        [::core::mem::offset_of!(mfxFrameData__bindgen_ty_4, Cb) - 0usize];
    ["Offset of field: mfxFrameData__bindgen_ty_4::U"][::core::mem::offset_of!(mfxFrameData__bindgen_ty_4, U) - 0usize];
    ["Offset of field: mfxFrameData__bindgen_ty_4::U16"]
        [::core::mem::offset_of!(mfxFrameData__bindgen_ty_4, U16) - 0usize];
    ["Offset of field: mfxFrameData__bindgen_ty_4::G"][::core::mem::offset_of!(mfxFrameData__bindgen_ty_4, G) - 0usize];
    ["Offset of field: mfxFrameData__bindgen_ty_4::Y410"]
        [::core::mem::offset_of!(mfxFrameData__bindgen_ty_4, Y410) - 0usize];
    ["Offset of field: mfxFrameData__bindgen_ty_4::Y416"]
        [::core::mem::offset_of!(mfxFrameData__bindgen_ty_4, Y416) - 0usize];
};
#[repr(C)]
#[derive(Copy, Clone)]
pub union mfxFrameData__bindgen_ty_5 {
    pub Cr: *mut mfxU8,
    pub V: *mut mfxU8,
    pub V16: *mut mfxU16,
    pub B: *mut mfxU8,
    pub A2RGB10: *mut mfxA2RGB10,
    pub ABGRFP16: *mut mfxABGR16FP,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of mfxFrameData__bindgen_ty_5"][::core::mem::size_of::<mfxFrameData__bindgen_ty_5>() - 8usize];
    ["Alignment of mfxFrameData__bindgen_ty_5"][::core::mem::align_of::<mfxFrameData__bindgen_ty_5>() - 8usize];
    ["Offset of field: mfxFrameData__bindgen_ty_5::Cr"]
        [::core::mem::offset_of!(mfxFrameData__bindgen_ty_5, Cr) - 0usize];
    ["Offset of field: mfxFrameData__bindgen_ty_5::V"][::core::mem::offset_of!(mfxFrameData__bindgen_ty_5, V) - 0usize];
    ["Offset of field: mfxFrameData__bindgen_ty_5::V16"]
        [::core::mem::offset_of!(mfxFrameData__bindgen_ty_5, V16) - 0usize];
    ["Offset of field: mfxFrameData__bindgen_ty_5::B"][::core::mem::offset_of!(mfxFrameData__bindgen_ty_5, B) - 0usize];
    ["Offset of field: mfxFrameData__bindgen_ty_5::A2RGB10"]
        [::core::mem::offset_of!(mfxFrameData__bindgen_ty_5, A2RGB10) - 0usize];
    ["Offset of field: mfxFrameData__bindgen_ty_5::ABGRFP16"]
        [::core::mem::offset_of!(mfxFrameData__bindgen_ty_5, ABGRFP16) - 0usize];
};
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of mfxFrameData"][::core::mem::size_of::<mfxFrameData>() - 96usize];
    ["Alignment of mfxFrameData"][::core::mem::align_of::<mfxFrameData>() - 8usize];
    ["Offset of field: mfxFrameData::NumExtParam"][::core::mem::offset_of!(mfxFrameData, NumExtParam) - 8usize];
    ["Offset of field: mfxFrameData::reserved"][::core::mem::offset_of!(mfxFrameData, reserved) - 10usize];
    ["Offset of field: mfxFrameData::MemType"][::core::mem::offset_of!(mfxFrameData, MemType) - 28usize];
    ["Offset of field: mfxFrameData::PitchHigh"][::core::mem::offset_of!(mfxFrameData, PitchHigh) - 30usize];
    ["Offset of field: mfxFrameData::TimeStamp"][::core::mem::offset_of!(mfxFrameData, TimeStamp) - 32usize];
    ["Offset of field: mfxFrameData::FrameOrder"][::core::mem::offset_of!(mfxFrameData, FrameOrder) - 40usize];
    ["Offset of field: mfxFrameData::Locked"][::core::mem::offset_of!(mfxFrameData, Locked) - 44usize];
    ["Offset of field: mfxFrameData::A"][::core::mem::offset_of!(mfxFrameData, A) - 72usize];
    ["Offset of field: mfxFrameData::MemId"][::core::mem::offset_of!(mfxFrameData, MemId) - 80usize];
    ["Offset of field: mfxFrameData::Corrupted"][::core::mem::offset_of!(mfxFrameData, Corrupted) - 88usize];
    ["Offset of field: mfxFrameData::DataFlag"][::core::mem::offset_of!(mfxFrameData, DataFlag) - 90usize];
};
pub const MFX_HANDLE_DIRECT3D_DEVICE_MANAGER9: mfxHandleType = 1;
pub const MFX_HANDLE_D3D9_DEVICE_MANAGER: mfxHandleType = 1;
pub const MFX_HANDLE_RESERVED1: mfxHandleType = 2;
pub const MFX_HANDLE_D3D11_DEVICE: mfxHandleType = 3;
pub const MFX_HANDLE_VA_DISPLAY: mfxHandleType = 4;
pub const MFX_HANDLE_RESERVED3: mfxHandleType = 5;
pub const MFX_HANDLE_VA_CONFIG_ID: mfxHandleType = 6;
pub const MFX_HANDLE_VA_CONTEXT_ID: mfxHandleType = 7;
pub const MFX_HANDLE_CM_DEVICE: mfxHandleType = 8;
pub const MFX_HANDLE_HDDLUNITE_WORKLOADCONTEXT: mfxHandleType = 9;
pub const MFX_HANDLE_PXP_CONTEXT: mfxHandleType = 10;
pub const MFX_HANDLE_CONFIG_INTERFACE: mfxHandleType = 1000;
pub type mfxHandleType = ::core::ffi::c_int;
#[repr(C)]
#[derive(Copy, Clone)]
pub struct mfxFrameSurface1 {
    pub __bindgen_anon_1: mfxFrameSurface1__bindgen_ty_1,
    pub Version: mfxStructVersion,
    pub reserved1: [mfxU16; 3usize],
    pub Info: mfxFrameInfo,
    pub Data: mfxFrameData,
}
#[repr(C)]
#[derive(Copy, Clone)]
pub union mfxFrameSurface1__bindgen_ty_1 {
    pub FrameInterface: *mut mfxFrameSurfaceInterface,
    pub reserved: [mfxU32; 2usize],
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of mfxFrameSurface1__bindgen_ty_1"][::core::mem::size_of::<mfxFrameSurface1__bindgen_ty_1>() - 8usize];
    ["Alignment of mfxFrameSurface1__bindgen_ty_1"][::core::mem::align_of::<mfxFrameSurface1__bindgen_ty_1>() - 8usize];
    ["Offset of field: mfxFrameSurface1__bindgen_ty_1::FrameInterface"]
        [::core::mem::offset_of!(mfxFrameSurface1__bindgen_ty_1, FrameInterface) - 0usize];
    ["Offset of field: mfxFrameSurface1__bindgen_ty_1::reserved"]
        [::core::mem::offset_of!(mfxFrameSurface1__bindgen_ty_1, reserved) - 0usize];
};
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of mfxFrameSurface1"][::core::mem::size_of::<mfxFrameSurface1>() - 184usize];
    ["Alignment of mfxFrameSurface1"][::core::mem::align_of::<mfxFrameSurface1>() - 8usize];
    ["Offset of field: mfxFrameSurface1::Version"][::core::mem::offset_of!(mfxFrameSurface1, Version) - 8usize];
    ["Offset of field: mfxFrameSurface1::reserved1"][::core::mem::offset_of!(mfxFrameSurface1, reserved1) - 10usize];
    ["Offset of field: mfxFrameSurface1::Info"][::core::mem::offset_of!(mfxFrameSurface1, Info) - 16usize];
    ["Offset of field: mfxFrameSurface1::Data"][::core::mem::offset_of!(mfxFrameSurface1, Data) - 88usize];
};
#[repr(C)]
#[derive(Copy, Clone)]
pub struct mfxFrameSurfaceInterface {
    pub Context: mfxHDL,
    pub Version: mfxStructVersion,
    pub reserved1: [mfxU16; 3usize],
    pub AddRef: ::core::option::Option<unsafe extern "C" fn(surface: *mut mfxFrameSurface1) -> mfxStatus>,
    pub Release: ::core::option::Option<unsafe extern "C" fn(surface: *mut mfxFrameSurface1) -> mfxStatus>,
    pub GetRefCounter:
        ::core::option::Option<unsafe extern "C" fn(surface: *mut mfxFrameSurface1, counter: *mut mfxU32) -> mfxStatus>,
    pub Map: ::core::option::Option<unsafe extern "C" fn(surface: *mut mfxFrameSurface1, flags: mfxU32) -> mfxStatus>,
    pub Unmap: ::core::option::Option<unsafe extern "C" fn(surface: *mut mfxFrameSurface1) -> mfxStatus>,
    pub GetNativeHandle: ::core::option::Option<
        unsafe extern "C" fn(
            surface: *mut mfxFrameSurface1,
            resource: *mut mfxHDL,
            resource_type: *mut mfxResourceType,
        ) -> mfxStatus,
    >,
    pub GetDeviceHandle: ::core::option::Option<
        unsafe extern "C" fn(
            surface: *mut mfxFrameSurface1,
            device_handle: *mut mfxHDL,
            device_type: *mut mfxHandleType,
        ) -> mfxStatus,
    >,
    pub Synchronize:
        ::core::option::Option<unsafe extern "C" fn(surface: *mut mfxFrameSurface1, wait: mfxU32) -> mfxStatus>,
    pub OnComplete: ::core::option::Option<unsafe extern "C" fn(sts: mfxStatus)>,
    pub QueryInterface: ::core::option::Option<
        unsafe extern "C" fn(surface: *mut mfxFrameSurface1, guid: mfxGUID, iface: *mut mfxHDL) -> mfxStatus,
    >,
    pub reserved2: [mfxHDL; 2usize],
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of mfxFrameSurfaceInterface"][::core::mem::size_of::<mfxFrameSurfaceInterface>() - 112usize];
    ["Alignment of mfxFrameSurfaceInterface"][::core::mem::align_of::<mfxFrameSurfaceInterface>() - 8usize];
    ["Offset of field: mfxFrameSurfaceInterface::Context"]
        [::core::mem::offset_of!(mfxFrameSurfaceInterface, Context) - 0usize];
    ["Offset of field: mfxFrameSurfaceInterface::Version"]
        [::core::mem::offset_of!(mfxFrameSurfaceInterface, Version) - 8usize];
    ["Offset of field: mfxFrameSurfaceInterface::reserved1"]
        [::core::mem::offset_of!(mfxFrameSurfaceInterface, reserved1) - 10usize];
    ["Offset of field: mfxFrameSurfaceInterface::AddRef"]
        [::core::mem::offset_of!(mfxFrameSurfaceInterface, AddRef) - 16usize];
    ["Offset of field: mfxFrameSurfaceInterface::Release"]
        [::core::mem::offset_of!(mfxFrameSurfaceInterface, Release) - 24usize];
    ["Offset of field: mfxFrameSurfaceInterface::GetRefCounter"]
        [::core::mem::offset_of!(mfxFrameSurfaceInterface, GetRefCounter) - 32usize];
    ["Offset of field: mfxFrameSurfaceInterface::Map"]
        [::core::mem::offset_of!(mfxFrameSurfaceInterface, Map) - 40usize];
    ["Offset of field: mfxFrameSurfaceInterface::Unmap"]
        [::core::mem::offset_of!(mfxFrameSurfaceInterface, Unmap) - 48usize];
    ["Offset of field: mfxFrameSurfaceInterface::GetNativeHandle"]
        [::core::mem::offset_of!(mfxFrameSurfaceInterface, GetNativeHandle) - 56usize];
    ["Offset of field: mfxFrameSurfaceInterface::GetDeviceHandle"]
        [::core::mem::offset_of!(mfxFrameSurfaceInterface, GetDeviceHandle) - 64usize];
    ["Offset of field: mfxFrameSurfaceInterface::Synchronize"]
        [::core::mem::offset_of!(mfxFrameSurfaceInterface, Synchronize) - 72usize];
    ["Offset of field: mfxFrameSurfaceInterface::OnComplete"]
        [::core::mem::offset_of!(mfxFrameSurfaceInterface, OnComplete) - 80usize];
    ["Offset of field: mfxFrameSurfaceInterface::QueryInterface"]
        [::core::mem::offset_of!(mfxFrameSurfaceInterface, QueryInterface) - 88usize];
    ["Offset of field: mfxFrameSurfaceInterface::reserved2"]
        [::core::mem::offset_of!(mfxFrameSurfaceInterface, reserved2) - 96usize];
};
pub const MFX_TIMESTAMPCALC_UNKNOWN: _bindgen_ty_12 = 0;
pub const MFX_TIMESTAMPCALC_TELECINE: _bindgen_ty_12 = 1;
pub type _bindgen_ty_12 = ::core::ffi::c_int;
#[repr(C)]
#[derive(Copy, Clone)]
pub struct mfxInfoMFX {
    pub reserved: [mfxU32; 7usize],
    pub LowPower: mfxU16,
    pub BRCParamMultiplier: mfxU16,
    pub FrameInfo: mfxFrameInfo,
    pub CodecId: mfxU32,
    pub CodecProfile: mfxU16,
    pub CodecLevel: mfxU16,
    pub NumThread: mfxU16,
    pub __bindgen_anon_1: mfxInfoMFX__bindgen_ty_1,
}
#[repr(C)]
#[derive(Copy, Clone)]
pub union mfxInfoMFX__bindgen_ty_1 {
    pub __bindgen_anon_1: mfxInfoMFX__bindgen_ty_1__bindgen_ty_1,
    pub __bindgen_anon_2: mfxInfoMFX__bindgen_ty_1__bindgen_ty_2,
    pub __bindgen_anon_3: mfxInfoMFX__bindgen_ty_1__bindgen_ty_3,
    pub __bindgen_anon_4: mfxInfoMFX__bindgen_ty_1__bindgen_ty_4,
}
#[repr(C)]
#[derive(Copy, Clone)]
pub struct mfxInfoMFX__bindgen_ty_1__bindgen_ty_1 {
    pub TargetUsage: mfxU16,
    pub GopPicSize: mfxU16,
    pub GopRefDist: mfxU16,
    pub GopOptFlag: mfxU16,
    pub IdrInterval: mfxU16,
    pub RateControlMethod: mfxU16,
    pub __bindgen_anon_1: mfxInfoMFX__bindgen_ty_1__bindgen_ty_1__bindgen_ty_1,
    pub BufferSizeInKB: mfxU16,
    pub __bindgen_anon_2: mfxInfoMFX__bindgen_ty_1__bindgen_ty_1__bindgen_ty_2,
    pub __bindgen_anon_3: mfxInfoMFX__bindgen_ty_1__bindgen_ty_1__bindgen_ty_3,
    pub NumSlice: mfxU16,
    pub NumRefFrame: mfxU16,
    pub EncodedOrder: mfxU16,
}
#[repr(C)]
#[derive(Copy, Clone)]
pub union mfxInfoMFX__bindgen_ty_1__bindgen_ty_1__bindgen_ty_1 {
    pub InitialDelayInKB: mfxU16,
    pub QPI: mfxU16,
    pub Accuracy: mfxU16,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of mfxInfoMFX__bindgen_ty_1__bindgen_ty_1__bindgen_ty_1"]
        [::core::mem::size_of::<mfxInfoMFX__bindgen_ty_1__bindgen_ty_1__bindgen_ty_1>() - 2usize];
    ["Alignment of mfxInfoMFX__bindgen_ty_1__bindgen_ty_1__bindgen_ty_1"]
        [::core::mem::align_of::<mfxInfoMFX__bindgen_ty_1__bindgen_ty_1__bindgen_ty_1>() - 2usize];
    ["Offset of field: mfxInfoMFX__bindgen_ty_1__bindgen_ty_1__bindgen_ty_1::InitialDelayInKB"]
        [::core::mem::offset_of!(mfxInfoMFX__bindgen_ty_1__bindgen_ty_1__bindgen_ty_1, InitialDelayInKB) - 0usize];
    ["Offset of field: mfxInfoMFX__bindgen_ty_1__bindgen_ty_1__bindgen_ty_1::QPI"]
        [::core::mem::offset_of!(mfxInfoMFX__bindgen_ty_1__bindgen_ty_1__bindgen_ty_1, QPI) - 0usize];
    ["Offset of field: mfxInfoMFX__bindgen_ty_1__bindgen_ty_1__bindgen_ty_1::Accuracy"]
        [::core::mem::offset_of!(mfxInfoMFX__bindgen_ty_1__bindgen_ty_1__bindgen_ty_1, Accuracy) - 0usize];
};
#[repr(C)]
#[derive(Copy, Clone)]
pub union mfxInfoMFX__bindgen_ty_1__bindgen_ty_1__bindgen_ty_2 {
    pub TargetKbps: mfxU16,
    pub QPP: mfxU16,
    pub ICQQuality: mfxU16,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of mfxInfoMFX__bindgen_ty_1__bindgen_ty_1__bindgen_ty_2"]
        [::core::mem::size_of::<mfxInfoMFX__bindgen_ty_1__bindgen_ty_1__bindgen_ty_2>() - 2usize];
    ["Alignment of mfxInfoMFX__bindgen_ty_1__bindgen_ty_1__bindgen_ty_2"]
        [::core::mem::align_of::<mfxInfoMFX__bindgen_ty_1__bindgen_ty_1__bindgen_ty_2>() - 2usize];
    ["Offset of field: mfxInfoMFX__bindgen_ty_1__bindgen_ty_1__bindgen_ty_2::TargetKbps"]
        [::core::mem::offset_of!(mfxInfoMFX__bindgen_ty_1__bindgen_ty_1__bindgen_ty_2, TargetKbps) - 0usize];
    ["Offset of field: mfxInfoMFX__bindgen_ty_1__bindgen_ty_1__bindgen_ty_2::QPP"]
        [::core::mem::offset_of!(mfxInfoMFX__bindgen_ty_1__bindgen_ty_1__bindgen_ty_2, QPP) - 0usize];
    ["Offset of field: mfxInfoMFX__bindgen_ty_1__bindgen_ty_1__bindgen_ty_2::ICQQuality"]
        [::core::mem::offset_of!(mfxInfoMFX__bindgen_ty_1__bindgen_ty_1__bindgen_ty_2, ICQQuality) - 0usize];
};
#[repr(C)]
#[derive(Copy, Clone)]
pub union mfxInfoMFX__bindgen_ty_1__bindgen_ty_1__bindgen_ty_3 {
    pub MaxKbps: mfxU16,
    pub QPB: mfxU16,
    pub Convergence: mfxU16,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of mfxInfoMFX__bindgen_ty_1__bindgen_ty_1__bindgen_ty_3"]
        [::core::mem::size_of::<mfxInfoMFX__bindgen_ty_1__bindgen_ty_1__bindgen_ty_3>() - 2usize];
    ["Alignment of mfxInfoMFX__bindgen_ty_1__bindgen_ty_1__bindgen_ty_3"]
        [::core::mem::align_of::<mfxInfoMFX__bindgen_ty_1__bindgen_ty_1__bindgen_ty_3>() - 2usize];
    ["Offset of field: mfxInfoMFX__bindgen_ty_1__bindgen_ty_1__bindgen_ty_3::MaxKbps"]
        [::core::mem::offset_of!(mfxInfoMFX__bindgen_ty_1__bindgen_ty_1__bindgen_ty_3, MaxKbps) - 0usize];
    ["Offset of field: mfxInfoMFX__bindgen_ty_1__bindgen_ty_1__bindgen_ty_3::QPB"]
        [::core::mem::offset_of!(mfxInfoMFX__bindgen_ty_1__bindgen_ty_1__bindgen_ty_3, QPB) - 0usize];
    ["Offset of field: mfxInfoMFX__bindgen_ty_1__bindgen_ty_1__bindgen_ty_3::Convergence"]
        [::core::mem::offset_of!(mfxInfoMFX__bindgen_ty_1__bindgen_ty_1__bindgen_ty_3, Convergence) - 0usize];
};
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of mfxInfoMFX__bindgen_ty_1__bindgen_ty_1"]
        [::core::mem::size_of::<mfxInfoMFX__bindgen_ty_1__bindgen_ty_1>() - 26usize];
    ["Alignment of mfxInfoMFX__bindgen_ty_1__bindgen_ty_1"]
        [::core::mem::align_of::<mfxInfoMFX__bindgen_ty_1__bindgen_ty_1>() - 2usize];
    ["Offset of field: mfxInfoMFX__bindgen_ty_1__bindgen_ty_1::TargetUsage"]
        [::core::mem::offset_of!(mfxInfoMFX__bindgen_ty_1__bindgen_ty_1, TargetUsage) - 0usize];
    ["Offset of field: mfxInfoMFX__bindgen_ty_1__bindgen_ty_1::GopPicSize"]
        [::core::mem::offset_of!(mfxInfoMFX__bindgen_ty_1__bindgen_ty_1, GopPicSize) - 2usize];
    ["Offset of field: mfxInfoMFX__bindgen_ty_1__bindgen_ty_1::GopRefDist"]
        [::core::mem::offset_of!(mfxInfoMFX__bindgen_ty_1__bindgen_ty_1, GopRefDist) - 4usize];
    ["Offset of field: mfxInfoMFX__bindgen_ty_1__bindgen_ty_1::GopOptFlag"]
        [::core::mem::offset_of!(mfxInfoMFX__bindgen_ty_1__bindgen_ty_1, GopOptFlag) - 6usize];
    ["Offset of field: mfxInfoMFX__bindgen_ty_1__bindgen_ty_1::IdrInterval"]
        [::core::mem::offset_of!(mfxInfoMFX__bindgen_ty_1__bindgen_ty_1, IdrInterval) - 8usize];
    ["Offset of field: mfxInfoMFX__bindgen_ty_1__bindgen_ty_1::RateControlMethod"]
        [::core::mem::offset_of!(mfxInfoMFX__bindgen_ty_1__bindgen_ty_1, RateControlMethod) - 10usize];
    ["Offset of field: mfxInfoMFX__bindgen_ty_1__bindgen_ty_1::BufferSizeInKB"]
        [::core::mem::offset_of!(mfxInfoMFX__bindgen_ty_1__bindgen_ty_1, BufferSizeInKB) - 14usize];
    ["Offset of field: mfxInfoMFX__bindgen_ty_1__bindgen_ty_1::NumSlice"]
        [::core::mem::offset_of!(mfxInfoMFX__bindgen_ty_1__bindgen_ty_1, NumSlice) - 20usize];
    ["Offset of field: mfxInfoMFX__bindgen_ty_1__bindgen_ty_1::NumRefFrame"]
        [::core::mem::offset_of!(mfxInfoMFX__bindgen_ty_1__bindgen_ty_1, NumRefFrame) - 22usize];
    ["Offset of field: mfxInfoMFX__bindgen_ty_1__bindgen_ty_1::EncodedOrder"]
        [::core::mem::offset_of!(mfxInfoMFX__bindgen_ty_1__bindgen_ty_1, EncodedOrder) - 24usize];
};
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct mfxInfoMFX__bindgen_ty_1__bindgen_ty_2 {
    pub DecodedOrder: mfxU16,
    pub ExtendedPicStruct: mfxU16,
    pub TimeStampCalc: mfxU16,
    pub SliceGroupsPresent: mfxU16,
    pub MaxDecFrameBuffering: mfxU16,
    pub EnableReallocRequest: mfxU16,
    pub FilmGrain: mfxU16,
    pub IgnoreLevelConstrain: mfxU16,
    pub SkipOutput: mfxU16,
    pub reserved2: [mfxU16; 4usize],
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of mfxInfoMFX__bindgen_ty_1__bindgen_ty_2"]
        [::core::mem::size_of::<mfxInfoMFX__bindgen_ty_1__bindgen_ty_2>() - 26usize];
    ["Alignment of mfxInfoMFX__bindgen_ty_1__bindgen_ty_2"]
        [::core::mem::align_of::<mfxInfoMFX__bindgen_ty_1__bindgen_ty_2>() - 2usize];
    ["Offset of field: mfxInfoMFX__bindgen_ty_1__bindgen_ty_2::DecodedOrder"]
        [::core::mem::offset_of!(mfxInfoMFX__bindgen_ty_1__bindgen_ty_2, DecodedOrder) - 0usize];
    ["Offset of field: mfxInfoMFX__bindgen_ty_1__bindgen_ty_2::ExtendedPicStruct"]
        [::core::mem::offset_of!(mfxInfoMFX__bindgen_ty_1__bindgen_ty_2, ExtendedPicStruct) - 2usize];
    ["Offset of field: mfxInfoMFX__bindgen_ty_1__bindgen_ty_2::TimeStampCalc"]
        [::core::mem::offset_of!(mfxInfoMFX__bindgen_ty_1__bindgen_ty_2, TimeStampCalc) - 4usize];
    ["Offset of field: mfxInfoMFX__bindgen_ty_1__bindgen_ty_2::SliceGroupsPresent"]
        [::core::mem::offset_of!(mfxInfoMFX__bindgen_ty_1__bindgen_ty_2, SliceGroupsPresent) - 6usize];
    ["Offset of field: mfxInfoMFX__bindgen_ty_1__bindgen_ty_2::MaxDecFrameBuffering"]
        [::core::mem::offset_of!(mfxInfoMFX__bindgen_ty_1__bindgen_ty_2, MaxDecFrameBuffering) - 8usize];
    ["Offset of field: mfxInfoMFX__bindgen_ty_1__bindgen_ty_2::EnableReallocRequest"]
        [::core::mem::offset_of!(mfxInfoMFX__bindgen_ty_1__bindgen_ty_2, EnableReallocRequest) - 10usize];
    ["Offset of field: mfxInfoMFX__bindgen_ty_1__bindgen_ty_2::FilmGrain"]
        [::core::mem::offset_of!(mfxInfoMFX__bindgen_ty_1__bindgen_ty_2, FilmGrain) - 12usize];
    ["Offset of field: mfxInfoMFX__bindgen_ty_1__bindgen_ty_2::IgnoreLevelConstrain"]
        [::core::mem::offset_of!(mfxInfoMFX__bindgen_ty_1__bindgen_ty_2, IgnoreLevelConstrain) - 14usize];
    ["Offset of field: mfxInfoMFX__bindgen_ty_1__bindgen_ty_2::SkipOutput"]
        [::core::mem::offset_of!(mfxInfoMFX__bindgen_ty_1__bindgen_ty_2, SkipOutput) - 16usize];
    ["Offset of field: mfxInfoMFX__bindgen_ty_1__bindgen_ty_2::reserved2"]
        [::core::mem::offset_of!(mfxInfoMFX__bindgen_ty_1__bindgen_ty_2, reserved2) - 18usize];
};
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct mfxInfoMFX__bindgen_ty_1__bindgen_ty_3 {
    pub JPEGChromaFormat: mfxU16,
    pub Rotation: mfxU16,
    pub JPEGColorFormat: mfxU16,
    pub InterleavedDec: mfxU16,
    pub SamplingFactorH: [mfxU8; 4usize],
    pub SamplingFactorV: [mfxU8; 4usize],
    pub reserved3: [mfxU16; 5usize],
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of mfxInfoMFX__bindgen_ty_1__bindgen_ty_3"]
        [::core::mem::size_of::<mfxInfoMFX__bindgen_ty_1__bindgen_ty_3>() - 26usize];
    ["Alignment of mfxInfoMFX__bindgen_ty_1__bindgen_ty_3"]
        [::core::mem::align_of::<mfxInfoMFX__bindgen_ty_1__bindgen_ty_3>() - 2usize];
    ["Offset of field: mfxInfoMFX__bindgen_ty_1__bindgen_ty_3::JPEGChromaFormat"]
        [::core::mem::offset_of!(mfxInfoMFX__bindgen_ty_1__bindgen_ty_3, JPEGChromaFormat) - 0usize];
    ["Offset of field: mfxInfoMFX__bindgen_ty_1__bindgen_ty_3::Rotation"]
        [::core::mem::offset_of!(mfxInfoMFX__bindgen_ty_1__bindgen_ty_3, Rotation) - 2usize];
    ["Offset of field: mfxInfoMFX__bindgen_ty_1__bindgen_ty_3::JPEGColorFormat"]
        [::core::mem::offset_of!(mfxInfoMFX__bindgen_ty_1__bindgen_ty_3, JPEGColorFormat) - 4usize];
    ["Offset of field: mfxInfoMFX__bindgen_ty_1__bindgen_ty_3::InterleavedDec"]
        [::core::mem::offset_of!(mfxInfoMFX__bindgen_ty_1__bindgen_ty_3, InterleavedDec) - 6usize];
    ["Offset of field: mfxInfoMFX__bindgen_ty_1__bindgen_ty_3::SamplingFactorH"]
        [::core::mem::offset_of!(mfxInfoMFX__bindgen_ty_1__bindgen_ty_3, SamplingFactorH) - 8usize];
    ["Offset of field: mfxInfoMFX__bindgen_ty_1__bindgen_ty_3::SamplingFactorV"]
        [::core::mem::offset_of!(mfxInfoMFX__bindgen_ty_1__bindgen_ty_3, SamplingFactorV) - 12usize];
    ["Offset of field: mfxInfoMFX__bindgen_ty_1__bindgen_ty_3::reserved3"]
        [::core::mem::offset_of!(mfxInfoMFX__bindgen_ty_1__bindgen_ty_3, reserved3) - 16usize];
};
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct mfxInfoMFX__bindgen_ty_1__bindgen_ty_4 {
    pub Interleaved: mfxU16,
    pub Quality: mfxU16,
    pub RestartInterval: mfxU16,
    pub reserved5: [mfxU16; 10usize],
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of mfxInfoMFX__bindgen_ty_1__bindgen_ty_4"]
        [::core::mem::size_of::<mfxInfoMFX__bindgen_ty_1__bindgen_ty_4>() - 26usize];
    ["Alignment of mfxInfoMFX__bindgen_ty_1__bindgen_ty_4"]
        [::core::mem::align_of::<mfxInfoMFX__bindgen_ty_1__bindgen_ty_4>() - 2usize];
    ["Offset of field: mfxInfoMFX__bindgen_ty_1__bindgen_ty_4::Interleaved"]
        [::core::mem::offset_of!(mfxInfoMFX__bindgen_ty_1__bindgen_ty_4, Interleaved) - 0usize];
    ["Offset of field: mfxInfoMFX__bindgen_ty_1__bindgen_ty_4::Quality"]
        [::core::mem::offset_of!(mfxInfoMFX__bindgen_ty_1__bindgen_ty_4, Quality) - 2usize];
    ["Offset of field: mfxInfoMFX__bindgen_ty_1__bindgen_ty_4::RestartInterval"]
        [::core::mem::offset_of!(mfxInfoMFX__bindgen_ty_1__bindgen_ty_4, RestartInterval) - 4usize];
    ["Offset of field: mfxInfoMFX__bindgen_ty_1__bindgen_ty_4::reserved5"]
        [::core::mem::offset_of!(mfxInfoMFX__bindgen_ty_1__bindgen_ty_4, reserved5) - 6usize];
};
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of mfxInfoMFX__bindgen_ty_1"][::core::mem::size_of::<mfxInfoMFX__bindgen_ty_1>() - 26usize];
    ["Alignment of mfxInfoMFX__bindgen_ty_1"][::core::mem::align_of::<mfxInfoMFX__bindgen_ty_1>() - 2usize];
};
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of mfxInfoMFX"][::core::mem::size_of::<mfxInfoMFX>() - 136usize];
    ["Alignment of mfxInfoMFX"][::core::mem::align_of::<mfxInfoMFX>() - 4usize];
    ["Offset of field: mfxInfoMFX::reserved"][::core::mem::offset_of!(mfxInfoMFX, reserved) - 0usize];
    ["Offset of field: mfxInfoMFX::LowPower"][::core::mem::offset_of!(mfxInfoMFX, LowPower) - 28usize];
    ["Offset of field: mfxInfoMFX::BRCParamMultiplier"]
        [::core::mem::offset_of!(mfxInfoMFX, BRCParamMultiplier) - 30usize];
    ["Offset of field: mfxInfoMFX::FrameInfo"][::core::mem::offset_of!(mfxInfoMFX, FrameInfo) - 32usize];
    ["Offset of field: mfxInfoMFX::CodecId"][::core::mem::offset_of!(mfxInfoMFX, CodecId) - 100usize];
    ["Offset of field: mfxInfoMFX::CodecProfile"][::core::mem::offset_of!(mfxInfoMFX, CodecProfile) - 104usize];
    ["Offset of field: mfxInfoMFX::CodecLevel"][::core::mem::offset_of!(mfxInfoMFX, CodecLevel) - 106usize];
    ["Offset of field: mfxInfoMFX::NumThread"][::core::mem::offset_of!(mfxInfoMFX, NumThread) - 108usize];
};
#[repr(C)]
#[derive(Copy, Clone)]
pub struct mfxInfoVPP {
    pub reserved: [mfxU32; 8usize],
    pub In: mfxFrameInfo,
    pub Out: mfxFrameInfo,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of mfxInfoVPP"][::core::mem::size_of::<mfxInfoVPP>() - 168usize];
    ["Alignment of mfxInfoVPP"][::core::mem::align_of::<mfxInfoVPP>() - 4usize];
    ["Offset of field: mfxInfoVPP::reserved"][::core::mem::offset_of!(mfxInfoVPP, reserved) - 0usize];
    ["Offset of field: mfxInfoVPP::In"][::core::mem::offset_of!(mfxInfoVPP, In) - 32usize];
    ["Offset of field: mfxInfoVPP::Out"][::core::mem::offset_of!(mfxInfoVPP, Out) - 100usize];
};
#[repr(C)]
#[derive(Copy, Clone)]
pub struct mfxVideoParam {
    pub AllocId: mfxU32,
    pub reserved: [mfxU32; 2usize],
    pub reserved3: mfxU16,
    pub AsyncDepth: mfxU16,
    pub __bindgen_anon_1: mfxVideoParam__bindgen_ty_1,
    pub Protected: mfxU16,
    pub IOPattern: mfxU16,
    pub ExtParam: *mut *mut mfxExtBuffer,
    pub NumExtParam: mfxU16,
    pub reserved2: mfxU16,
}
#[repr(C)]
#[derive(Copy, Clone)]
pub union mfxVideoParam__bindgen_ty_1 {
    pub mfx: mfxInfoMFX,
    pub vpp: mfxInfoVPP,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of mfxVideoParam__bindgen_ty_1"][::core::mem::size_of::<mfxVideoParam__bindgen_ty_1>() - 168usize];
    ["Alignment of mfxVideoParam__bindgen_ty_1"][::core::mem::align_of::<mfxVideoParam__bindgen_ty_1>() - 4usize];
    ["Offset of field: mfxVideoParam__bindgen_ty_1::mfx"]
        [::core::mem::offset_of!(mfxVideoParam__bindgen_ty_1, mfx) - 0usize];
    ["Offset of field: mfxVideoParam__bindgen_ty_1::vpp"]
        [::core::mem::offset_of!(mfxVideoParam__bindgen_ty_1, vpp) - 0usize];
};
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of mfxVideoParam"][::core::mem::size_of::<mfxVideoParam>() - 208usize];
    ["Alignment of mfxVideoParam"][::core::mem::align_of::<mfxVideoParam>() - 8usize];
    ["Offset of field: mfxVideoParam::AllocId"][::core::mem::offset_of!(mfxVideoParam, AllocId) - 0usize];
    ["Offset of field: mfxVideoParam::reserved"][::core::mem::offset_of!(mfxVideoParam, reserved) - 4usize];
    ["Offset of field: mfxVideoParam::reserved3"][::core::mem::offset_of!(mfxVideoParam, reserved3) - 12usize];
    ["Offset of field: mfxVideoParam::AsyncDepth"][::core::mem::offset_of!(mfxVideoParam, AsyncDepth) - 14usize];
    ["Offset of field: mfxVideoParam::Protected"][::core::mem::offset_of!(mfxVideoParam, Protected) - 184usize];
    ["Offset of field: mfxVideoParam::IOPattern"][::core::mem::offset_of!(mfxVideoParam, IOPattern) - 186usize];
    ["Offset of field: mfxVideoParam::ExtParam"][::core::mem::offset_of!(mfxVideoParam, ExtParam) - 192usize];
    ["Offset of field: mfxVideoParam::NumExtParam"][::core::mem::offset_of!(mfxVideoParam, NumExtParam) - 200usize];
    ["Offset of field: mfxVideoParam::reserved2"][::core::mem::offset_of!(mfxVideoParam, reserved2) - 202usize];
};
pub const MFX_IOPATTERN_IN_VIDEO_MEMORY: _bindgen_ty_13 = 1;
pub const MFX_IOPATTERN_IN_SYSTEM_MEMORY: _bindgen_ty_13 = 2;
pub const MFX_IOPATTERN_OUT_VIDEO_MEMORY: _bindgen_ty_13 = 16;
pub const MFX_IOPATTERN_OUT_SYSTEM_MEMORY: _bindgen_ty_13 = 32;
pub type _bindgen_ty_13 = ::core::ffi::c_int;
pub const MFX_CODEC_AVC: _bindgen_ty_14 = 541283905;
pub const MFX_CODEC_HEVC: _bindgen_ty_14 = 1129727304;
pub const MFX_CODEC_MPEG2: _bindgen_ty_14 = 843534413;
pub const MFX_CODEC_VC1: _bindgen_ty_14 = 540099414;
pub const MFX_CODEC_CAPTURE: _bindgen_ty_14 = 1414545731;
pub const MFX_CODEC_VP9: _bindgen_ty_14 = 540627030;
pub const MFX_CODEC_AV1: _bindgen_ty_14 = 540104257;
pub const MFX_CODEC_VVC: _bindgen_ty_14 = 541283926;
pub type _bindgen_ty_14 = ::core::ffi::c_int;
pub const MFX_PROFILE_UNKNOWN: _bindgen_ty_15 = 0;
pub const MFX_LEVEL_UNKNOWN: _bindgen_ty_15 = 0;
pub const MFX_PROFILE_AVC_CONSTRAINT_SET0: _bindgen_ty_15 = 256;
pub const MFX_PROFILE_AVC_CONSTRAINT_SET1: _bindgen_ty_15 = 512;
pub const MFX_PROFILE_AVC_CONSTRAINT_SET2: _bindgen_ty_15 = 1024;
pub const MFX_PROFILE_AVC_CONSTRAINT_SET3: _bindgen_ty_15 = 2048;
pub const MFX_PROFILE_AVC_CONSTRAINT_SET4: _bindgen_ty_15 = 4096;
pub const MFX_PROFILE_AVC_CONSTRAINT_SET5: _bindgen_ty_15 = 8192;
pub const MFX_PROFILE_AVC_BASELINE: _bindgen_ty_15 = 66;
pub const MFX_PROFILE_AVC_MAIN: _bindgen_ty_15 = 77;
pub const MFX_PROFILE_AVC_EXTENDED: _bindgen_ty_15 = 88;
pub const MFX_PROFILE_AVC_HIGH: _bindgen_ty_15 = 100;
pub const MFX_PROFILE_AVC_HIGH10: _bindgen_ty_15 = 110;
pub const MFX_PROFILE_AVC_HIGH_422: _bindgen_ty_15 = 122;
pub const MFX_PROFILE_AVC_CONSTRAINED_BASELINE: _bindgen_ty_15 = 578;
pub const MFX_PROFILE_AVC_CONSTRAINED_HIGH: _bindgen_ty_15 = 12388;
pub const MFX_PROFILE_AVC_PROGRESSIVE_HIGH: _bindgen_ty_15 = 4196;
pub const MFX_LEVEL_AVC_1: _bindgen_ty_15 = 10;
pub const MFX_LEVEL_AVC_1b: _bindgen_ty_15 = 9;
pub const MFX_LEVEL_AVC_11: _bindgen_ty_15 = 11;
pub const MFX_LEVEL_AVC_12: _bindgen_ty_15 = 12;
pub const MFX_LEVEL_AVC_13: _bindgen_ty_15 = 13;
pub const MFX_LEVEL_AVC_2: _bindgen_ty_15 = 20;
pub const MFX_LEVEL_AVC_21: _bindgen_ty_15 = 21;
pub const MFX_LEVEL_AVC_22: _bindgen_ty_15 = 22;
pub const MFX_LEVEL_AVC_3: _bindgen_ty_15 = 30;
pub const MFX_LEVEL_AVC_31: _bindgen_ty_15 = 31;
pub const MFX_LEVEL_AVC_32: _bindgen_ty_15 = 32;
pub const MFX_LEVEL_AVC_4: _bindgen_ty_15 = 40;
pub const MFX_LEVEL_AVC_41: _bindgen_ty_15 = 41;
pub const MFX_LEVEL_AVC_42: _bindgen_ty_15 = 42;
pub const MFX_LEVEL_AVC_5: _bindgen_ty_15 = 50;
pub const MFX_LEVEL_AVC_51: _bindgen_ty_15 = 51;
pub const MFX_LEVEL_AVC_52: _bindgen_ty_15 = 52;
pub const MFX_LEVEL_AVC_6: _bindgen_ty_15 = 60;
pub const MFX_LEVEL_AVC_61: _bindgen_ty_15 = 61;
pub const MFX_LEVEL_AVC_62: _bindgen_ty_15 = 62;
pub const MFX_PROFILE_MPEG2_SIMPLE: _bindgen_ty_15 = 80;
pub const MFX_PROFILE_MPEG2_MAIN: _bindgen_ty_15 = 64;
pub const MFX_PROFILE_MPEG2_HIGH: _bindgen_ty_15 = 16;
pub const MFX_LEVEL_MPEG2_LOW: _bindgen_ty_15 = 10;
pub const MFX_LEVEL_MPEG2_MAIN: _bindgen_ty_15 = 8;
pub const MFX_LEVEL_MPEG2_HIGH: _bindgen_ty_15 = 4;
pub const MFX_LEVEL_MPEG2_HIGH1440: _bindgen_ty_15 = 6;
pub const MFX_PROFILE_VC1_SIMPLE: _bindgen_ty_15 = 1;
pub const MFX_PROFILE_VC1_MAIN: _bindgen_ty_15 = 5;
pub const MFX_PROFILE_VC1_ADVANCED: _bindgen_ty_15 = 13;
pub const MFX_LEVEL_VC1_LOW: _bindgen_ty_15 = 1;
pub const MFX_LEVEL_VC1_MEDIAN: _bindgen_ty_15 = 3;
pub const MFX_LEVEL_VC1_HIGH: _bindgen_ty_15 = 5;
pub const MFX_LEVEL_VC1_0: _bindgen_ty_15 = 1;
pub const MFX_LEVEL_VC1_1: _bindgen_ty_15 = 2;
pub const MFX_LEVEL_VC1_2: _bindgen_ty_15 = 3;
pub const MFX_LEVEL_VC1_3: _bindgen_ty_15 = 4;
pub const MFX_LEVEL_VC1_4: _bindgen_ty_15 = 5;
pub const MFX_PROFILE_HEVC_MAIN: _bindgen_ty_15 = 1;
pub const MFX_PROFILE_HEVC_MAIN10: _bindgen_ty_15 = 2;
pub const MFX_PROFILE_HEVC_MAINSP: _bindgen_ty_15 = 3;
pub const MFX_PROFILE_HEVC_REXT: _bindgen_ty_15 = 4;
pub const MFX_PROFILE_HEVC_SCC: _bindgen_ty_15 = 9;
pub const MFX_LEVEL_HEVC_1: _bindgen_ty_15 = 10;
pub const MFX_LEVEL_HEVC_2: _bindgen_ty_15 = 20;
pub const MFX_LEVEL_HEVC_21: _bindgen_ty_15 = 21;
pub const MFX_LEVEL_HEVC_3: _bindgen_ty_15 = 30;
pub const MFX_LEVEL_HEVC_31: _bindgen_ty_15 = 31;
pub const MFX_LEVEL_HEVC_4: _bindgen_ty_15 = 40;
pub const MFX_LEVEL_HEVC_41: _bindgen_ty_15 = 41;
pub const MFX_LEVEL_HEVC_5: _bindgen_ty_15 = 50;
pub const MFX_LEVEL_HEVC_51: _bindgen_ty_15 = 51;
pub const MFX_LEVEL_HEVC_52: _bindgen_ty_15 = 52;
pub const MFX_LEVEL_HEVC_6: _bindgen_ty_15 = 60;
pub const MFX_LEVEL_HEVC_61: _bindgen_ty_15 = 61;
pub const MFX_LEVEL_HEVC_62: _bindgen_ty_15 = 62;
pub const MFX_LEVEL_HEVC_85: _bindgen_ty_15 = 85;
pub const MFX_TIER_HEVC_MAIN: _bindgen_ty_15 = 0;
pub const MFX_TIER_HEVC_HIGH: _bindgen_ty_15 = 256;
pub const MFX_PROFILE_VP9_0: _bindgen_ty_15 = 1;
pub const MFX_PROFILE_VP9_1: _bindgen_ty_15 = 2;
pub const MFX_PROFILE_VP9_2: _bindgen_ty_15 = 3;
pub const MFX_PROFILE_VP9_3: _bindgen_ty_15 = 4;
pub const MFX_PROFILE_AV1_MAIN: _bindgen_ty_15 = 1;
pub const MFX_PROFILE_AV1_HIGH: _bindgen_ty_15 = 2;
pub const MFX_PROFILE_AV1_PRO: _bindgen_ty_15 = 3;
pub const MFX_LEVEL_AV1_2: _bindgen_ty_15 = 20;
pub const MFX_LEVEL_AV1_21: _bindgen_ty_15 = 21;
pub const MFX_LEVEL_AV1_22: _bindgen_ty_15 = 22;
pub const MFX_LEVEL_AV1_23: _bindgen_ty_15 = 23;
pub const MFX_LEVEL_AV1_3: _bindgen_ty_15 = 30;
pub const MFX_LEVEL_AV1_31: _bindgen_ty_15 = 31;
pub const MFX_LEVEL_AV1_32: _bindgen_ty_15 = 32;
pub const MFX_LEVEL_AV1_33: _bindgen_ty_15 = 33;
pub const MFX_LEVEL_AV1_4: _bindgen_ty_15 = 40;
pub const MFX_LEVEL_AV1_41: _bindgen_ty_15 = 41;
pub const MFX_LEVEL_AV1_42: _bindgen_ty_15 = 42;
pub const MFX_LEVEL_AV1_43: _bindgen_ty_15 = 43;
pub const MFX_LEVEL_AV1_5: _bindgen_ty_15 = 50;
pub const MFX_LEVEL_AV1_51: _bindgen_ty_15 = 51;
pub const MFX_LEVEL_AV1_52: _bindgen_ty_15 = 52;
pub const MFX_LEVEL_AV1_53: _bindgen_ty_15 = 53;
pub const MFX_LEVEL_AV1_6: _bindgen_ty_15 = 60;
pub const MFX_LEVEL_AV1_61: _bindgen_ty_15 = 61;
pub const MFX_LEVEL_AV1_62: _bindgen_ty_15 = 62;
pub const MFX_LEVEL_AV1_63: _bindgen_ty_15 = 63;
pub const MFX_LEVEL_AV1_7: _bindgen_ty_15 = 70;
pub const MFX_LEVEL_AV1_71: _bindgen_ty_15 = 71;
pub const MFX_LEVEL_AV1_72: _bindgen_ty_15 = 72;
pub const MFX_LEVEL_AV1_73: _bindgen_ty_15 = 73;
pub const MFX_PROFILE_VVC_MAIN10: _bindgen_ty_15 = 1;
pub const MFX_PROFILE_VVC_MAIN10_STILL_PICTURE: _bindgen_ty_15 = 65;
pub const MFX_LEVEL_VVC_1: _bindgen_ty_15 = 16;
pub const MFX_LEVEL_VVC_2: _bindgen_ty_15 = 32;
pub const MFX_LEVEL_VVC_21: _bindgen_ty_15 = 35;
pub const MFX_LEVEL_VVC_3: _bindgen_ty_15 = 48;
pub const MFX_LEVEL_VVC_31: _bindgen_ty_15 = 51;
pub const MFX_LEVEL_VVC_4: _bindgen_ty_15 = 64;
pub const MFX_LEVEL_VVC_41: _bindgen_ty_15 = 67;
pub const MFX_LEVEL_VVC_5: _bindgen_ty_15 = 80;
pub const MFX_LEVEL_VVC_51: _bindgen_ty_15 = 83;
pub const MFX_LEVEL_VVC_52: _bindgen_ty_15 = 86;
pub const MFX_LEVEL_VVC_6: _bindgen_ty_15 = 96;
pub const MFX_LEVEL_VVC_61: _bindgen_ty_15 = 99;
pub const MFX_LEVEL_VVC_62: _bindgen_ty_15 = 102;
pub const MFX_LEVEL_VVC_63: _bindgen_ty_15 = 105;
pub const MFX_LEVEL_VVC_155: _bindgen_ty_15 = 255;
pub const MFX_TIER_VVC_MAIN: _bindgen_ty_15 = 0;
pub const MFX_TIER_VVC_HIGH: _bindgen_ty_15 = 256;
pub type _bindgen_ty_15 = ::core::ffi::c_int;
pub const MFX_GOP_CLOSED: _bindgen_ty_16 = 1;
pub const MFX_GOP_STRICT: _bindgen_ty_16 = 2;
pub type _bindgen_ty_16 = ::core::ffi::c_int;
pub const MFX_TARGETUSAGE_1: _bindgen_ty_17 = 1;
pub const MFX_TARGETUSAGE_2: _bindgen_ty_17 = 2;
pub const MFX_TARGETUSAGE_3: _bindgen_ty_17 = 3;
pub const MFX_TARGETUSAGE_4: _bindgen_ty_17 = 4;
pub const MFX_TARGETUSAGE_5: _bindgen_ty_17 = 5;
pub const MFX_TARGETUSAGE_6: _bindgen_ty_17 = 6;
pub const MFX_TARGETUSAGE_7: _bindgen_ty_17 = 7;
pub const MFX_TARGETUSAGE_UNKNOWN: _bindgen_ty_17 = 0;
pub const MFX_TARGETUSAGE_BEST_QUALITY: _bindgen_ty_17 = 1;
pub const MFX_TARGETUSAGE_BALANCED: _bindgen_ty_17 = 4;
pub const MFX_TARGETUSAGE_BEST_SPEED: _bindgen_ty_17 = 7;
pub type _bindgen_ty_17 = ::core::ffi::c_int;
pub const MFX_RATECONTROL_CBR: _bindgen_ty_18 = 1;
pub const MFX_RATECONTROL_VBR: _bindgen_ty_18 = 2;
pub const MFX_RATECONTROL_CQP: _bindgen_ty_18 = 3;
pub const MFX_RATECONTROL_AVBR: _bindgen_ty_18 = 4;
pub const MFX_RATECONTROL_RESERVED1: _bindgen_ty_18 = 5;
pub const MFX_RATECONTROL_RESERVED2: _bindgen_ty_18 = 6;
pub const MFX_RATECONTROL_RESERVED3: _bindgen_ty_18 = 100;
pub const MFX_RATECONTROL_RESERVED4: _bindgen_ty_18 = 7;
pub const MFX_RATECONTROL_LA: _bindgen_ty_18 = 8;
pub const MFX_RATECONTROL_ICQ: _bindgen_ty_18 = 9;
pub const MFX_RATECONTROL_VCM: _bindgen_ty_18 = 10;
pub const MFX_RATECONTROL_LA_ICQ: _bindgen_ty_18 = 11;
pub const MFX_RATECONTROL_LA_HRD: _bindgen_ty_18 = 13;
pub const MFX_RATECONTROL_QVBR: _bindgen_ty_18 = 14;
pub type _bindgen_ty_18 = ::core::ffi::c_int;
pub const MFX_TRELLIS_UNKNOWN: _bindgen_ty_19 = 0;
pub const MFX_TRELLIS_OFF: _bindgen_ty_19 = 1;
pub const MFX_TRELLIS_I: _bindgen_ty_19 = 2;
pub const MFX_TRELLIS_P: _bindgen_ty_19 = 4;
pub const MFX_TRELLIS_B: _bindgen_ty_19 = 8;
pub type _bindgen_ty_19 = ::core::ffi::c_int;
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct mfxExtCodingOption {
    pub Header: mfxExtBuffer,
    pub reserved1: mfxU16,
    pub RateDistortionOpt: mfxU16,
    pub MECostType: mfxU16,
    pub MESearchType: mfxU16,
    pub MVSearchWindow: mfxI16Pair,
    pub EndOfSequence: mfxU16,
    pub FramePicture: mfxU16,
    pub CAVLC: mfxU16,
    pub reserved2: [mfxU16; 2usize],
    pub RecoveryPointSEI: mfxU16,
    pub ViewOutput: mfxU16,
    pub NalHrdConformance: mfxU16,
    pub SingleSeiNalUnit: mfxU16,
    pub VuiVclHrdParameters: mfxU16,
    pub RefPicListReordering: mfxU16,
    pub ResetRefList: mfxU16,
    pub RefPicMarkRep: mfxU16,
    pub FieldOutput: mfxU16,
    pub IntraPredBlockSize: mfxU16,
    pub InterPredBlockSize: mfxU16,
    pub MVPrecision: mfxU16,
    pub MaxDecFrameBuffering: mfxU16,
    pub AUDelimiter: mfxU16,
    pub EndOfStream: mfxU16,
    pub PicTimingSEI: mfxU16,
    pub VuiNalHrdParameters: mfxU16,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of mfxExtCodingOption"][::core::mem::size_of::<mfxExtCodingOption>() - 64usize];
    ["Alignment of mfxExtCodingOption"][::core::mem::align_of::<mfxExtCodingOption>() - 4usize];
    ["Offset of field: mfxExtCodingOption::Header"][::core::mem::offset_of!(mfxExtCodingOption, Header) - 0usize];
    ["Offset of field: mfxExtCodingOption::reserved1"][::core::mem::offset_of!(mfxExtCodingOption, reserved1) - 8usize];
    ["Offset of field: mfxExtCodingOption::RateDistortionOpt"]
        [::core::mem::offset_of!(mfxExtCodingOption, RateDistortionOpt) - 10usize];
    ["Offset of field: mfxExtCodingOption::MECostType"]
        [::core::mem::offset_of!(mfxExtCodingOption, MECostType) - 12usize];
    ["Offset of field: mfxExtCodingOption::MESearchType"]
        [::core::mem::offset_of!(mfxExtCodingOption, MESearchType) - 14usize];
    ["Offset of field: mfxExtCodingOption::MVSearchWindow"]
        [::core::mem::offset_of!(mfxExtCodingOption, MVSearchWindow) - 16usize];
    ["Offset of field: mfxExtCodingOption::EndOfSequence"]
        [::core::mem::offset_of!(mfxExtCodingOption, EndOfSequence) - 20usize];
    ["Offset of field: mfxExtCodingOption::FramePicture"]
        [::core::mem::offset_of!(mfxExtCodingOption, FramePicture) - 22usize];
    ["Offset of field: mfxExtCodingOption::CAVLC"][::core::mem::offset_of!(mfxExtCodingOption, CAVLC) - 24usize];
    ["Offset of field: mfxExtCodingOption::reserved2"]
        [::core::mem::offset_of!(mfxExtCodingOption, reserved2) - 26usize];
    ["Offset of field: mfxExtCodingOption::RecoveryPointSEI"]
        [::core::mem::offset_of!(mfxExtCodingOption, RecoveryPointSEI) - 30usize];
    ["Offset of field: mfxExtCodingOption::ViewOutput"]
        [::core::mem::offset_of!(mfxExtCodingOption, ViewOutput) - 32usize];
    ["Offset of field: mfxExtCodingOption::NalHrdConformance"]
        [::core::mem::offset_of!(mfxExtCodingOption, NalHrdConformance) - 34usize];
    ["Offset of field: mfxExtCodingOption::SingleSeiNalUnit"]
        [::core::mem::offset_of!(mfxExtCodingOption, SingleSeiNalUnit) - 36usize];
    ["Offset of field: mfxExtCodingOption::VuiVclHrdParameters"]
        [::core::mem::offset_of!(mfxExtCodingOption, VuiVclHrdParameters) - 38usize];
    ["Offset of field: mfxExtCodingOption::RefPicListReordering"]
        [::core::mem::offset_of!(mfxExtCodingOption, RefPicListReordering) - 40usize];
    ["Offset of field: mfxExtCodingOption::ResetRefList"]
        [::core::mem::offset_of!(mfxExtCodingOption, ResetRefList) - 42usize];
    ["Offset of field: mfxExtCodingOption::RefPicMarkRep"]
        [::core::mem::offset_of!(mfxExtCodingOption, RefPicMarkRep) - 44usize];
    ["Offset of field: mfxExtCodingOption::FieldOutput"]
        [::core::mem::offset_of!(mfxExtCodingOption, FieldOutput) - 46usize];
    ["Offset of field: mfxExtCodingOption::IntraPredBlockSize"]
        [::core::mem::offset_of!(mfxExtCodingOption, IntraPredBlockSize) - 48usize];
    ["Offset of field: mfxExtCodingOption::InterPredBlockSize"]
        [::core::mem::offset_of!(mfxExtCodingOption, InterPredBlockSize) - 50usize];
    ["Offset of field: mfxExtCodingOption::MVPrecision"]
        [::core::mem::offset_of!(mfxExtCodingOption, MVPrecision) - 52usize];
    ["Offset of field: mfxExtCodingOption::MaxDecFrameBuffering"]
        [::core::mem::offset_of!(mfxExtCodingOption, MaxDecFrameBuffering) - 54usize];
    ["Offset of field: mfxExtCodingOption::AUDelimiter"]
        [::core::mem::offset_of!(mfxExtCodingOption, AUDelimiter) - 56usize];
    ["Offset of field: mfxExtCodingOption::EndOfStream"]
        [::core::mem::offset_of!(mfxExtCodingOption, EndOfStream) - 58usize];
    ["Offset of field: mfxExtCodingOption::PicTimingSEI"]
        [::core::mem::offset_of!(mfxExtCodingOption, PicTimingSEI) - 60usize];
    ["Offset of field: mfxExtCodingOption::VuiNalHrdParameters"]
        [::core::mem::offset_of!(mfxExtCodingOption, VuiNalHrdParameters) - 62usize];
};
pub const MFX_B_REF_UNKNOWN: _bindgen_ty_20 = 0;
pub const MFX_B_REF_OFF: _bindgen_ty_20 = 1;
pub const MFX_B_REF_PYRAMID: _bindgen_ty_20 = 2;
pub type _bindgen_ty_20 = ::core::ffi::c_int;
pub const MFX_LOOKAHEAD_DS_UNKNOWN: _bindgen_ty_21 = 0;
pub const MFX_LOOKAHEAD_DS_OFF: _bindgen_ty_21 = 1;
pub const MFX_LOOKAHEAD_DS_2x: _bindgen_ty_21 = 2;
pub const MFX_LOOKAHEAD_DS_4x: _bindgen_ty_21 = 3;
pub type _bindgen_ty_21 = ::core::ffi::c_int;
pub const MFX_BPSEI_DEFAULT: _bindgen_ty_22 = 0;
pub const MFX_BPSEI_IFRAME: _bindgen_ty_22 = 1;
pub type _bindgen_ty_22 = ::core::ffi::c_int;
pub const MFX_SKIPFRAME_NO_SKIP: _bindgen_ty_23 = 0;
pub const MFX_SKIPFRAME_INSERT_DUMMY: _bindgen_ty_23 = 1;
pub const MFX_SKIPFRAME_INSERT_NOTHING: _bindgen_ty_23 = 2;
pub const MFX_SKIPFRAME_BRC_ONLY: _bindgen_ty_23 = 3;
pub type _bindgen_ty_23 = ::core::ffi::c_int;
pub const MFX_REFRESH_NO: _bindgen_ty_24 = 0;
pub const MFX_REFRESH_VERTICAL: _bindgen_ty_24 = 1;
pub const MFX_REFRESH_HORIZONTAL: _bindgen_ty_24 = 2;
pub const MFX_REFRESH_SLICE: _bindgen_ty_24 = 3;
pub type _bindgen_ty_24 = ::core::ffi::c_int;
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct mfxExtCodingOption2 {
    pub Header: mfxExtBuffer,
    pub IntRefType: mfxU16,
    pub IntRefCycleSize: mfxU16,
    pub IntRefQPDelta: mfxI16,
    pub MaxFrameSize: mfxU32,
    pub MaxSliceSize: mfxU32,
    pub BitrateLimit: mfxU16,
    pub MBBRC: mfxU16,
    pub ExtBRC: mfxU16,
    pub LookAheadDepth: mfxU16,
    pub Trellis: mfxU16,
    pub RepeatPPS: mfxU16,
    pub BRefType: mfxU16,
    pub AdaptiveI: mfxU16,
    pub AdaptiveB: mfxU16,
    pub LookAheadDS: mfxU16,
    pub NumMbPerSlice: mfxU16,
    pub SkipFrame: mfxU16,
    pub MinQPI: mfxU8,
    pub MaxQPI: mfxU8,
    pub MinQPP: mfxU8,
    pub MaxQPP: mfxU8,
    pub MinQPB: mfxU8,
    pub MaxQPB: mfxU8,
    pub FixedFrameRate: mfxU16,
    pub DisableDeblockingIdc: mfxU16,
    pub DisableVUI: mfxU16,
    pub BufferingPeriodSEI: mfxU16,
    pub EnableMAD: mfxU16,
    pub UseRawRef: mfxU16,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of mfxExtCodingOption2"][::core::mem::size_of::<mfxExtCodingOption2>() - 68usize];
    ["Alignment of mfxExtCodingOption2"][::core::mem::align_of::<mfxExtCodingOption2>() - 4usize];
    ["Offset of field: mfxExtCodingOption2::Header"][::core::mem::offset_of!(mfxExtCodingOption2, Header) - 0usize];
    ["Offset of field: mfxExtCodingOption2::IntRefType"]
        [::core::mem::offset_of!(mfxExtCodingOption2, IntRefType) - 8usize];
    ["Offset of field: mfxExtCodingOption2::IntRefCycleSize"]
        [::core::mem::offset_of!(mfxExtCodingOption2, IntRefCycleSize) - 10usize];
    ["Offset of field: mfxExtCodingOption2::IntRefQPDelta"]
        [::core::mem::offset_of!(mfxExtCodingOption2, IntRefQPDelta) - 12usize];
    ["Offset of field: mfxExtCodingOption2::MaxFrameSize"]
        [::core::mem::offset_of!(mfxExtCodingOption2, MaxFrameSize) - 16usize];
    ["Offset of field: mfxExtCodingOption2::MaxSliceSize"]
        [::core::mem::offset_of!(mfxExtCodingOption2, MaxSliceSize) - 20usize];
    ["Offset of field: mfxExtCodingOption2::BitrateLimit"]
        [::core::mem::offset_of!(mfxExtCodingOption2, BitrateLimit) - 24usize];
    ["Offset of field: mfxExtCodingOption2::MBBRC"][::core::mem::offset_of!(mfxExtCodingOption2, MBBRC) - 26usize];
    ["Offset of field: mfxExtCodingOption2::ExtBRC"][::core::mem::offset_of!(mfxExtCodingOption2, ExtBRC) - 28usize];
    ["Offset of field: mfxExtCodingOption2::LookAheadDepth"]
        [::core::mem::offset_of!(mfxExtCodingOption2, LookAheadDepth) - 30usize];
    ["Offset of field: mfxExtCodingOption2::Trellis"][::core::mem::offset_of!(mfxExtCodingOption2, Trellis) - 32usize];
    ["Offset of field: mfxExtCodingOption2::RepeatPPS"]
        [::core::mem::offset_of!(mfxExtCodingOption2, RepeatPPS) - 34usize];
    ["Offset of field: mfxExtCodingOption2::BRefType"]
        [::core::mem::offset_of!(mfxExtCodingOption2, BRefType) - 36usize];
    ["Offset of field: mfxExtCodingOption2::AdaptiveI"]
        [::core::mem::offset_of!(mfxExtCodingOption2, AdaptiveI) - 38usize];
    ["Offset of field: mfxExtCodingOption2::AdaptiveB"]
        [::core::mem::offset_of!(mfxExtCodingOption2, AdaptiveB) - 40usize];
    ["Offset of field: mfxExtCodingOption2::LookAheadDS"]
        [::core::mem::offset_of!(mfxExtCodingOption2, LookAheadDS) - 42usize];
    ["Offset of field: mfxExtCodingOption2::NumMbPerSlice"]
        [::core::mem::offset_of!(mfxExtCodingOption2, NumMbPerSlice) - 44usize];
    ["Offset of field: mfxExtCodingOption2::SkipFrame"]
        [::core::mem::offset_of!(mfxExtCodingOption2, SkipFrame) - 46usize];
    ["Offset of field: mfxExtCodingOption2::MinQPI"][::core::mem::offset_of!(mfxExtCodingOption2, MinQPI) - 48usize];
    ["Offset of field: mfxExtCodingOption2::MaxQPI"][::core::mem::offset_of!(mfxExtCodingOption2, MaxQPI) - 49usize];
    ["Offset of field: mfxExtCodingOption2::MinQPP"][::core::mem::offset_of!(mfxExtCodingOption2, MinQPP) - 50usize];
    ["Offset of field: mfxExtCodingOption2::MaxQPP"][::core::mem::offset_of!(mfxExtCodingOption2, MaxQPP) - 51usize];
    ["Offset of field: mfxExtCodingOption2::MinQPB"][::core::mem::offset_of!(mfxExtCodingOption2, MinQPB) - 52usize];
    ["Offset of field: mfxExtCodingOption2::MaxQPB"][::core::mem::offset_of!(mfxExtCodingOption2, MaxQPB) - 53usize];
    ["Offset of field: mfxExtCodingOption2::FixedFrameRate"]
        [::core::mem::offset_of!(mfxExtCodingOption2, FixedFrameRate) - 54usize];
    ["Offset of field: mfxExtCodingOption2::DisableDeblockingIdc"]
        [::core::mem::offset_of!(mfxExtCodingOption2, DisableDeblockingIdc) - 56usize];
    ["Offset of field: mfxExtCodingOption2::DisableVUI"]
        [::core::mem::offset_of!(mfxExtCodingOption2, DisableVUI) - 58usize];
    ["Offset of field: mfxExtCodingOption2::BufferingPeriodSEI"]
        [::core::mem::offset_of!(mfxExtCodingOption2, BufferingPeriodSEI) - 60usize];
    ["Offset of field: mfxExtCodingOption2::EnableMAD"]
        [::core::mem::offset_of!(mfxExtCodingOption2, EnableMAD) - 62usize];
    ["Offset of field: mfxExtCodingOption2::UseRawRef"]
        [::core::mem::offset_of!(mfxExtCodingOption2, UseRawRef) - 64usize];
};
pub const MFX_WEIGHTED_PRED_UNKNOWN: _bindgen_ty_25 = 0;
pub const MFX_WEIGHTED_PRED_DEFAULT: _bindgen_ty_25 = 1;
pub const MFX_WEIGHTED_PRED_EXPLICIT: _bindgen_ty_25 = 2;
pub const MFX_WEIGHTED_PRED_IMPLICIT: _bindgen_ty_25 = 3;
pub type _bindgen_ty_25 = ::core::ffi::c_int;
pub const MFX_SCENARIO_UNKNOWN: _bindgen_ty_26 = 0;
pub const MFX_SCENARIO_DISPLAY_REMOTING: _bindgen_ty_26 = 1;
pub const MFX_SCENARIO_VIDEO_CONFERENCE: _bindgen_ty_26 = 2;
pub const MFX_SCENARIO_ARCHIVE: _bindgen_ty_26 = 3;
pub const MFX_SCENARIO_LIVE_STREAMING: _bindgen_ty_26 = 4;
pub const MFX_SCENARIO_CAMERA_CAPTURE: _bindgen_ty_26 = 5;
pub const MFX_SCENARIO_VIDEO_SURVEILLANCE: _bindgen_ty_26 = 6;
pub const MFX_SCENARIO_GAME_STREAMING: _bindgen_ty_26 = 7;
pub const MFX_SCENARIO_REMOTE_GAMING: _bindgen_ty_26 = 8;
pub type _bindgen_ty_26 = ::core::ffi::c_int;
pub const MFX_CONTENT_UNKNOWN: _bindgen_ty_27 = 0;
pub const MFX_CONTENT_FULL_SCREEN_VIDEO: _bindgen_ty_27 = 1;
pub const MFX_CONTENT_NON_VIDEO_SCREEN: _bindgen_ty_27 = 2;
pub const MFX_CONTENT_NOISY_VIDEO: _bindgen_ty_27 = 3;
pub type _bindgen_ty_27 = ::core::ffi::c_int;
pub const MFX_P_REF_DEFAULT: _bindgen_ty_28 = 0;
pub const MFX_P_REF_SIMPLE: _bindgen_ty_28 = 1;
pub const MFX_P_REF_PYRAMID: _bindgen_ty_28 = 2;
pub type _bindgen_ty_28 = ::core::ffi::c_int;
#[repr(C)]
#[derive(Copy, Clone)]
pub struct mfxExtCodingOption3 {
    pub Header: mfxExtBuffer,
    pub NumSliceI: mfxU16,
    pub NumSliceP: mfxU16,
    pub NumSliceB: mfxU16,
    pub WinBRCMaxAvgKbps: mfxU16,
    pub WinBRCSize: mfxU16,
    pub QVBRQuality: mfxU16,
    pub EnableMBQP: mfxU16,
    pub IntRefCycleDist: mfxU16,
    pub DirectBiasAdjustment: mfxU16,
    pub GlobalMotionBiasAdjustment: mfxU16,
    pub MVCostScalingFactor: mfxU16,
    pub MBDisableSkipMap: mfxU16,
    pub WeightedPred: mfxU16,
    pub WeightedBiPred: mfxU16,
    pub AspectRatioInfoPresent: mfxU16,
    pub OverscanInfoPresent: mfxU16,
    pub OverscanAppropriate: mfxU16,
    pub TimingInfoPresent: mfxU16,
    pub BitstreamRestriction: mfxU16,
    pub LowDelayHrd: mfxU16,
    pub MotionVectorsOverPicBoundaries: mfxU16,
    pub reserved1: [mfxU16; 2usize],
    pub ScenarioInfo: mfxU16,
    pub ContentInfo: mfxU16,
    pub PRefType: mfxU16,
    pub FadeDetection: mfxU16,
    pub reserved2: [mfxU16; 2usize],
    pub GPB: mfxU16,
    pub MaxFrameSizeI: mfxU32,
    pub MaxFrameSizeP: mfxU32,
    pub reserved3: [mfxU32; 3usize],
    pub EnableQPOffset: mfxU16,
    pub QPOffset: [mfxI16; 8usize],
    pub NumRefActiveP: [mfxU16; 8usize],
    pub NumRefActiveBL0: [mfxU16; 8usize],
    pub NumRefActiveBL1: [mfxU16; 8usize],
    pub reserved6: mfxU16,
    pub TransformSkip: mfxU16,
    pub TargetChromaFormatPlus1: mfxU16,
    pub TargetBitDepthLuma: mfxU16,
    pub TargetBitDepthChroma: mfxU16,
    pub BRCPanicMode: mfxU16,
    pub LowDelayBRC: mfxU16,
    pub EnableMBForceIntra: mfxU16,
    pub AdaptiveMaxFrameSize: mfxU16,
    pub RepartitionCheckEnable: mfxU16,
    pub reserved5: [mfxU16; 3usize],
    pub EncodedUnitsInfo: mfxU16,
    pub EnableNalUnitType: mfxU16,
    pub __bindgen_anon_1: mfxExtCodingOption3__bindgen_ty_1,
    pub AdaptiveCQM: mfxU16,
    pub AdaptiveRef: mfxU16,
    pub reserved: [mfxU16; 161usize],
}
#[repr(C)]
#[derive(Copy, Clone)]
pub union mfxExtCodingOption3__bindgen_ty_1 {
    pub ExtBrcAdaptiveLTR: mfxU16,
    pub AdaptiveLTR: mfxU16,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of mfxExtCodingOption3__bindgen_ty_1"][::core::mem::size_of::<mfxExtCodingOption3__bindgen_ty_1>() - 2usize];
    ["Alignment of mfxExtCodingOption3__bindgen_ty_1"]
        [::core::mem::align_of::<mfxExtCodingOption3__bindgen_ty_1>() - 2usize];
    ["Offset of field: mfxExtCodingOption3__bindgen_ty_1::ExtBrcAdaptiveLTR"]
        [::core::mem::offset_of!(mfxExtCodingOption3__bindgen_ty_1, ExtBrcAdaptiveLTR) - 0usize];
    ["Offset of field: mfxExtCodingOption3__bindgen_ty_1::AdaptiveLTR"]
        [::core::mem::offset_of!(mfxExtCodingOption3__bindgen_ty_1, AdaptiveLTR) - 0usize];
};
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of mfxExtCodingOption3"][::core::mem::size_of::<mfxExtCodingOption3>() - 512usize];
    ["Alignment of mfxExtCodingOption3"][::core::mem::align_of::<mfxExtCodingOption3>() - 4usize];
    ["Offset of field: mfxExtCodingOption3::Header"][::core::mem::offset_of!(mfxExtCodingOption3, Header) - 0usize];
    ["Offset of field: mfxExtCodingOption3::NumSliceI"]
        [::core::mem::offset_of!(mfxExtCodingOption3, NumSliceI) - 8usize];
    ["Offset of field: mfxExtCodingOption3::NumSliceP"]
        [::core::mem::offset_of!(mfxExtCodingOption3, NumSliceP) - 10usize];
    ["Offset of field: mfxExtCodingOption3::NumSliceB"]
        [::core::mem::offset_of!(mfxExtCodingOption3, NumSliceB) - 12usize];
    ["Offset of field: mfxExtCodingOption3::WinBRCMaxAvgKbps"]
        [::core::mem::offset_of!(mfxExtCodingOption3, WinBRCMaxAvgKbps) - 14usize];
    ["Offset of field: mfxExtCodingOption3::WinBRCSize"]
        [::core::mem::offset_of!(mfxExtCodingOption3, WinBRCSize) - 16usize];
    ["Offset of field: mfxExtCodingOption3::QVBRQuality"]
        [::core::mem::offset_of!(mfxExtCodingOption3, QVBRQuality) - 18usize];
    ["Offset of field: mfxExtCodingOption3::EnableMBQP"]
        [::core::mem::offset_of!(mfxExtCodingOption3, EnableMBQP) - 20usize];
    ["Offset of field: mfxExtCodingOption3::IntRefCycleDist"]
        [::core::mem::offset_of!(mfxExtCodingOption3, IntRefCycleDist) - 22usize];
    ["Offset of field: mfxExtCodingOption3::DirectBiasAdjustment"]
        [::core::mem::offset_of!(mfxExtCodingOption3, DirectBiasAdjustment) - 24usize];
    ["Offset of field: mfxExtCodingOption3::GlobalMotionBiasAdjustment"]
        [::core::mem::offset_of!(mfxExtCodingOption3, GlobalMotionBiasAdjustment) - 26usize];
    ["Offset of field: mfxExtCodingOption3::MVCostScalingFactor"]
        [::core::mem::offset_of!(mfxExtCodingOption3, MVCostScalingFactor) - 28usize];
    ["Offset of field: mfxExtCodingOption3::MBDisableSkipMap"]
        [::core::mem::offset_of!(mfxExtCodingOption3, MBDisableSkipMap) - 30usize];
    ["Offset of field: mfxExtCodingOption3::WeightedPred"]
        [::core::mem::offset_of!(mfxExtCodingOption3, WeightedPred) - 32usize];
    ["Offset of field: mfxExtCodingOption3::WeightedBiPred"]
        [::core::mem::offset_of!(mfxExtCodingOption3, WeightedBiPred) - 34usize];
    ["Offset of field: mfxExtCodingOption3::AspectRatioInfoPresent"]
        [::core::mem::offset_of!(mfxExtCodingOption3, AspectRatioInfoPresent) - 36usize];
    ["Offset of field: mfxExtCodingOption3::OverscanInfoPresent"]
        [::core::mem::offset_of!(mfxExtCodingOption3, OverscanInfoPresent) - 38usize];
    ["Offset of field: mfxExtCodingOption3::OverscanAppropriate"]
        [::core::mem::offset_of!(mfxExtCodingOption3, OverscanAppropriate) - 40usize];
    ["Offset of field: mfxExtCodingOption3::TimingInfoPresent"]
        [::core::mem::offset_of!(mfxExtCodingOption3, TimingInfoPresent) - 42usize];
    ["Offset of field: mfxExtCodingOption3::BitstreamRestriction"]
        [::core::mem::offset_of!(mfxExtCodingOption3, BitstreamRestriction) - 44usize];
    ["Offset of field: mfxExtCodingOption3::LowDelayHrd"]
        [::core::mem::offset_of!(mfxExtCodingOption3, LowDelayHrd) - 46usize];
    ["Offset of field: mfxExtCodingOption3::MotionVectorsOverPicBoundaries"]
        [::core::mem::offset_of!(mfxExtCodingOption3, MotionVectorsOverPicBoundaries) - 48usize];
    ["Offset of field: mfxExtCodingOption3::reserved1"]
        [::core::mem::offset_of!(mfxExtCodingOption3, reserved1) - 50usize];
    ["Offset of field: mfxExtCodingOption3::ScenarioInfo"]
        [::core::mem::offset_of!(mfxExtCodingOption3, ScenarioInfo) - 54usize];
    ["Offset of field: mfxExtCodingOption3::ContentInfo"]
        [::core::mem::offset_of!(mfxExtCodingOption3, ContentInfo) - 56usize];
    ["Offset of field: mfxExtCodingOption3::PRefType"]
        [::core::mem::offset_of!(mfxExtCodingOption3, PRefType) - 58usize];
    ["Offset of field: mfxExtCodingOption3::FadeDetection"]
        [::core::mem::offset_of!(mfxExtCodingOption3, FadeDetection) - 60usize];
    ["Offset of field: mfxExtCodingOption3::reserved2"]
        [::core::mem::offset_of!(mfxExtCodingOption3, reserved2) - 62usize];
    ["Offset of field: mfxExtCodingOption3::GPB"][::core::mem::offset_of!(mfxExtCodingOption3, GPB) - 66usize];
    ["Offset of field: mfxExtCodingOption3::MaxFrameSizeI"]
        [::core::mem::offset_of!(mfxExtCodingOption3, MaxFrameSizeI) - 68usize];
    ["Offset of field: mfxExtCodingOption3::MaxFrameSizeP"]
        [::core::mem::offset_of!(mfxExtCodingOption3, MaxFrameSizeP) - 72usize];
    ["Offset of field: mfxExtCodingOption3::reserved3"]
        [::core::mem::offset_of!(mfxExtCodingOption3, reserved3) - 76usize];
    ["Offset of field: mfxExtCodingOption3::EnableQPOffset"]
        [::core::mem::offset_of!(mfxExtCodingOption3, EnableQPOffset) - 88usize];
    ["Offset of field: mfxExtCodingOption3::QPOffset"]
        [::core::mem::offset_of!(mfxExtCodingOption3, QPOffset) - 90usize];
    ["Offset of field: mfxExtCodingOption3::NumRefActiveP"]
        [::core::mem::offset_of!(mfxExtCodingOption3, NumRefActiveP) - 106usize];
    ["Offset of field: mfxExtCodingOption3::NumRefActiveBL0"]
        [::core::mem::offset_of!(mfxExtCodingOption3, NumRefActiveBL0) - 122usize];
    ["Offset of field: mfxExtCodingOption3::NumRefActiveBL1"]
        [::core::mem::offset_of!(mfxExtCodingOption3, NumRefActiveBL1) - 138usize];
    ["Offset of field: mfxExtCodingOption3::reserved6"]
        [::core::mem::offset_of!(mfxExtCodingOption3, reserved6) - 154usize];
    ["Offset of field: mfxExtCodingOption3::TransformSkip"]
        [::core::mem::offset_of!(mfxExtCodingOption3, TransformSkip) - 156usize];
    ["Offset of field: mfxExtCodingOption3::TargetChromaFormatPlus1"]
        [::core::mem::offset_of!(mfxExtCodingOption3, TargetChromaFormatPlus1) - 158usize];
    ["Offset of field: mfxExtCodingOption3::TargetBitDepthLuma"]
        [::core::mem::offset_of!(mfxExtCodingOption3, TargetBitDepthLuma) - 160usize];
    ["Offset of field: mfxExtCodingOption3::TargetBitDepthChroma"]
        [::core::mem::offset_of!(mfxExtCodingOption3, TargetBitDepthChroma) - 162usize];
    ["Offset of field: mfxExtCodingOption3::BRCPanicMode"]
        [::core::mem::offset_of!(mfxExtCodingOption3, BRCPanicMode) - 164usize];
    ["Offset of field: mfxExtCodingOption3::LowDelayBRC"]
        [::core::mem::offset_of!(mfxExtCodingOption3, LowDelayBRC) - 166usize];
    ["Offset of field: mfxExtCodingOption3::EnableMBForceIntra"]
        [::core::mem::offset_of!(mfxExtCodingOption3, EnableMBForceIntra) - 168usize];
    ["Offset of field: mfxExtCodingOption3::AdaptiveMaxFrameSize"]
        [::core::mem::offset_of!(mfxExtCodingOption3, AdaptiveMaxFrameSize) - 170usize];
    ["Offset of field: mfxExtCodingOption3::RepartitionCheckEnable"]
        [::core::mem::offset_of!(mfxExtCodingOption3, RepartitionCheckEnable) - 172usize];
    ["Offset of field: mfxExtCodingOption3::reserved5"]
        [::core::mem::offset_of!(mfxExtCodingOption3, reserved5) - 174usize];
    ["Offset of field: mfxExtCodingOption3::EncodedUnitsInfo"]
        [::core::mem::offset_of!(mfxExtCodingOption3, EncodedUnitsInfo) - 180usize];
    ["Offset of field: mfxExtCodingOption3::EnableNalUnitType"]
        [::core::mem::offset_of!(mfxExtCodingOption3, EnableNalUnitType) - 182usize];
    ["Offset of field: mfxExtCodingOption3::AdaptiveCQM"]
        [::core::mem::offset_of!(mfxExtCodingOption3, AdaptiveCQM) - 186usize];
    ["Offset of field: mfxExtCodingOption3::AdaptiveRef"]
        [::core::mem::offset_of!(mfxExtCodingOption3, AdaptiveRef) - 188usize];
    ["Offset of field: mfxExtCodingOption3::reserved"]
        [::core::mem::offset_of!(mfxExtCodingOption3, reserved) - 190usize];
};
pub const MFX_BLOCKSIZE_UNKNOWN: _bindgen_ty_29 = 0;
pub const MFX_BLOCKSIZE_MIN_16X16: _bindgen_ty_29 = 1;
pub const MFX_BLOCKSIZE_MIN_8X8: _bindgen_ty_29 = 2;
pub const MFX_BLOCKSIZE_MIN_4X4: _bindgen_ty_29 = 3;
pub type _bindgen_ty_29 = ::core::ffi::c_int;
pub const MFX_MVPRECISION_UNKNOWN: _bindgen_ty_30 = 0;
pub const MFX_MVPRECISION_INTEGER: _bindgen_ty_30 = 1;
pub const MFX_MVPRECISION_HALFPEL: _bindgen_ty_30 = 2;
pub const MFX_MVPRECISION_QUARTERPEL: _bindgen_ty_30 = 4;
pub type _bindgen_ty_30 = ::core::ffi::c_int;
pub const MFX_CODINGOPTION_UNKNOWN: _bindgen_ty_31 = 0;
pub const MFX_CODINGOPTION_ON: _bindgen_ty_31 = 16;
pub const MFX_CODINGOPTION_OFF: _bindgen_ty_31 = 32;
pub const MFX_CODINGOPTION_ADAPTIVE: _bindgen_ty_31 = 48;
pub type _bindgen_ty_31 = ::core::ffi::c_int;
pub const MFX_BITSTREAM_NO_FLAG: _bindgen_ty_32 = 0;
pub const MFX_BITSTREAM_COMPLETE_FRAME: _bindgen_ty_32 = 1;
pub const MFX_BITSTREAM_EOS: _bindgen_ty_32 = 2;
pub type _bindgen_ty_32 = ::core::ffi::c_int;
pub const MFX_EXTBUFF_CODING_OPTION: _bindgen_ty_33 = 1347372099;
pub const MFX_EXTBUFF_CODING_OPTION_SPSPPS: _bindgen_ty_33 = 1347637059;
pub const MFX_EXTBUFF_VPP_DONOTUSE: _bindgen_ty_33 = 1163089230;
pub const MFX_EXTBUFF_VPP_AUXDATA: _bindgen_ty_33 = 1146639681;
pub const MFX_EXTBUFF_VPP_DENOISE2: _bindgen_ty_33 = 843664964;
pub const MFX_EXTBUFF_VPP_DENOISE: _bindgen_ty_33 = 1397313092;
pub const MFX_EXTBUFF_VPP_SCENE_ANALYSIS: _bindgen_ty_33 = 1498170195;
pub const MFX_EXTBUFF_VPP_SCENE_CHANGE: _bindgen_ty_33 = 1498170195;
pub const MFX_EXTBUFF_VPP_PROCAMP: _bindgen_ty_33 = 1347240272;
pub const MFX_EXTBUFF_VPP_DETAIL: _bindgen_ty_33 = 542393668;
pub const MFX_EXTBUFF_VIDEO_SIGNAL_INFO: _bindgen_ty_33 = 1313428310;
pub const MFX_EXTBUFF_VIDEO_SIGNAL_INFO_IN: _bindgen_ty_33 = 1229542230;
pub const MFX_EXTBUFF_VIDEO_SIGNAL_INFO_OUT: _bindgen_ty_33 = 1330205526;
pub const MFX_EXTBUFF_VPP_DOUSE: _bindgen_ty_33 = 1163089220;
pub const MFX_EXTBUFF_AVC_REFLIST_CTRL: _bindgen_ty_33 = 1414745170;
pub const MFX_EXTBUFF_VPP_FRAME_RATE_CONVERSION: _bindgen_ty_33 = 541282886;
pub const MFX_EXTBUFF_PICTURE_TIMING_SEI: _bindgen_ty_33 = 1163088976;
pub const MFX_EXTBUFF_AVC_TEMPORAL_LAYERS: _bindgen_ty_33 = 1280136257;
pub const MFX_EXTBUFF_CODING_OPTION2: _bindgen_ty_33 = 844055619;
pub const MFX_EXTBUFF_VPP_IMAGE_STABILIZATION: _bindgen_ty_33 = 1112822601;
pub const MFX_EXTBUFF_ENCODER_CAPABILITY: _bindgen_ty_33 = 1346588229;
pub const MFX_EXTBUFF_ENCODER_RESET_OPTION: _bindgen_ty_33 = 1330794053;
pub const MFX_EXTBUFF_ENCODED_FRAME_INFO: _bindgen_ty_33 = 1229344325;
pub const MFX_EXTBUFF_VPP_COMPOSITE: _bindgen_ty_33 = 1347240790;
pub const MFX_EXTBUFF_VPP_VIDEO_SIGNAL_INFO: _bindgen_ty_33 = 1230198358;
pub const MFX_EXTBUFF_ENCODER_ROI: _bindgen_ty_33 = 1229935173;
pub const MFX_EXTBUFF_VPP_DEINTERLACING: _bindgen_ty_33 = 1229213782;
pub const MFX_EXTBUFF_AVC_REFLISTS: _bindgen_ty_33 = 1398033490;
pub const MFX_EXTBUFF_DEC_VIDEO_PROCESSING: _bindgen_ty_33 = 1447249220;
pub const MFX_EXTBUFF_VPP_FIELD_PROCESSING: _bindgen_ty_33 = 1330794566;
pub const MFX_EXTBUFF_CODING_OPTION3: _bindgen_ty_33 = 860832835;
pub const MFX_EXTBUFF_CHROMA_LOC_INFO: _bindgen_ty_33 = 1313426499;
pub const MFX_EXTBUFF_MBQP: _bindgen_ty_33 = 1347502669;
pub const MFX_EXTBUFF_MB_FORCE_INTRA: _bindgen_ty_33 = 1229341261;
pub const MFX_EXTBUFF_HEVC_TILES: _bindgen_ty_33 = 1412773426;
pub const MFX_EXTBUFF_MB_DISABLE_SKIP_MAP: _bindgen_ty_33 = 1297302605;
pub const MFX_EXTBUFF_HEVC_PARAM: _bindgen_ty_33 = 1345664562;
pub const MFX_EXTBUFF_DECODED_FRAME_INFO: _bindgen_ty_33 = 1229342020;
pub const MFX_EXTBUFF_TIME_CODE: _bindgen_ty_33 = 1145261396;
pub const MFX_EXTBUFF_HEVC_REGION: _bindgen_ty_33 = 1379218994;
pub const MFX_EXTBUFF_PRED_WEIGHT_TABLE: _bindgen_ty_33 = 1415008325;
pub const MFX_EXTBUFF_DIRTY_RECTANGLES: _bindgen_ty_33 = 1229935172;
pub const MFX_EXTBUFF_MOVING_RECTANGLES: _bindgen_ty_33 = 1229935181;
pub const MFX_EXTBUFF_CODING_OPTION_VPS: _bindgen_ty_33 = 1347833667;
pub const MFX_EXTBUFF_VPP_ROTATION: _bindgen_ty_33 = 542396242;
pub const MFX_EXTBUFF_ENCODED_SLICES_INFO: _bindgen_ty_33 = 1230196293;
pub const MFX_EXTBUFF_VPP_SCALING: _bindgen_ty_33 = 1279480662;
pub const MFX_EXTBUFF_HEVC_REFLIST_CTRL: _bindgen_ty_33 = 1414745170;
pub const MFX_EXTBUFF_HEVC_REFLISTS: _bindgen_ty_33 = 1398033490;
pub const MFX_EXTBUFF_HEVC_TEMPORAL_LAYERS: _bindgen_ty_33 = 1280136257;
pub const MFX_EXTBUFF_VPP_MIRRORING: _bindgen_ty_33 = 1381124429;
pub const MFX_EXTBUFF_MV_OVER_PIC_BOUNDARIES: _bindgen_ty_33 = 1112561229;
pub const MFX_EXTBUFF_VPP_COLORFILL: _bindgen_ty_33 = 1179403094;
pub const MFX_EXTBUFF_DECODE_ERROR_REPORT: _bindgen_ty_33 = 1381123396;
pub const MFX_EXTBUFF_VPP_COLOR_CONVERSION: _bindgen_ty_33 = 1129530198;
pub const MFX_EXTBUFF_CONTENT_LIGHT_LEVEL_INFO: _bindgen_ty_33 = 1397312588;
pub const MFX_EXTBUFF_MASTERING_DISPLAY_COLOUR_VOLUME: _bindgen_ty_33 = 1398162244;
pub const MFX_EXTBUFF_MASTERING_DISPLAY_COLOUR_VOLUME_IN: _bindgen_ty_33 = 1230390084;
pub const MFX_EXTBUFF_MASTERING_DISPLAY_COLOUR_VOLUME_OUT: _bindgen_ty_33 = 1331053380;
pub const MFX_EXTBUFF_ENCODED_UNITS_INFO: _bindgen_ty_33 = 1230327365;
pub const MFX_EXTBUFF_VPP_MCTF: _bindgen_ty_33 = 1179927373;
pub const MFX_EXTBUFF_VP9_SEGMENTATION: _bindgen_ty_33 = 1195725625;
pub const MFX_EXTBUFF_VP9_TEMPORAL_LAYERS: _bindgen_ty_33 = 1280136249;
pub const MFX_EXTBUFF_VP9_PARAM: _bindgen_ty_33 = 1380012089;
pub const MFX_EXTBUFF_AVC_ROUNDING_OFFSET: _bindgen_ty_33 = 1329876562;
pub const MFX_EXTBUFF_PARTIAL_BITSTREAM_PARAM: _bindgen_ty_33 = 1347371600;
pub const MFX_EXTBUFF_ENCODER_IPCM_AREA: _bindgen_ty_33 = 1380795216;
pub const MFX_EXTBUFF_INSERT_HEADERS: _bindgen_ty_33 = 1163022419;
pub const MFX_EXTBUFF_DEVICE_AFFINITY_MASK: _bindgen_ty_33 = 1296449860;
pub const MFX_EXTBUFF_CROPS: _bindgen_ty_33 = 1347375683;
pub const MFX_EXTBUFF_AV1_BITSTREAM_PARAM: _bindgen_ty_33 = 1396846913;
pub const MFX_EXTBUFF_AV1_RESOLUTION_PARAM: _bindgen_ty_33 = 1397895489;
pub const MFX_EXTBUFF_AV1_TILE_PARAM: _bindgen_ty_33 = 1280586049;
pub const MFX_EXTBUFF_AV1_SEGMENTATION: _bindgen_ty_33 = 1195725617;
pub const MFX_EXTBUFF_AV1_FILM_GRAIN_PARAM: _bindgen_ty_33 = 1195782465;
pub const MFX_EXTBUFF_HYPER_MODE_PARAM: _bindgen_ty_33 = 1297111368;
pub const MFX_EXTBUFF_UNIVERSAL_TEMPORAL_LAYERS: _bindgen_ty_33 = 1347245141;
pub const MFX_EXTBUFF_UNIVERSAL_REFLIST_CTRL: _bindgen_ty_33 = 1414745170;
pub const MFX_EXTBUFF_VPP_3DLUT: _bindgen_ty_33 = 1414284372;
pub const MFX_EXTBUFF_ALLOCATION_HINTS: _bindgen_ty_33 = 1212369985;
pub const MFX_EXTBUFF_VPP_AI_SUPER_RESOLUTION: _bindgen_ty_33 = 1381187926;
pub const MFX_EXTBUFF_VPP_AI_FRAME_INTERPOLATION: _bindgen_ty_33 = 1229341014;
pub const MFX_EXTBUFF_ENCODED_QUALITY_INFO_MODE: _bindgen_ty_33 = 1297174085;
pub const MFX_EXTBUFF_ENCODED_QUALITY_INFO_OUTPUT: _bindgen_ty_33 = 1330728517;
pub const MFX_EXTBUFF_AV1_SCREEN_CONTENT_TOOLS: _bindgen_ty_33 = 1128485681;
pub const MFX_EXTBUFF_ALPHA_CHANNEL_ENC_CTRL: _bindgen_ty_33 = 1128612673;
pub const MFX_EXTBUFF_ALPHA_CHANNEL_SURFACE: _bindgen_ty_33 = 1179861825;
pub type _bindgen_ty_33 = ::core::ffi::c_int;
pub const MFX_PAYLOAD_CTRL_SUFFIX: _bindgen_ty_34 = 1;
pub type _bindgen_ty_34 = ::core::ffi::c_int;
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct mfxPayload {
    pub CtrlFlags: mfxU32,
    pub reserved: [mfxU32; 3usize],
    pub Data: *mut mfxU8,
    pub NumBit: mfxU32,
    pub Type: mfxU16,
    pub BufSize: mfxU16,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of mfxPayload"][::core::mem::size_of::<mfxPayload>() - 32usize];
    ["Alignment of mfxPayload"][::core::mem::align_of::<mfxPayload>() - 8usize];
    ["Offset of field: mfxPayload::CtrlFlags"][::core::mem::offset_of!(mfxPayload, CtrlFlags) - 0usize];
    ["Offset of field: mfxPayload::reserved"][::core::mem::offset_of!(mfxPayload, reserved) - 4usize];
    ["Offset of field: mfxPayload::Data"][::core::mem::offset_of!(mfxPayload, Data) - 16usize];
    ["Offset of field: mfxPayload::NumBit"][::core::mem::offset_of!(mfxPayload, NumBit) - 24usize];
    ["Offset of field: mfxPayload::Type"][::core::mem::offset_of!(mfxPayload, Type) - 28usize];
    ["Offset of field: mfxPayload::BufSize"][::core::mem::offset_of!(mfxPayload, BufSize) - 30usize];
};
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct mfxEncodeCtrl {
    pub Header: mfxExtBuffer,
    pub reserved: [mfxU32; 4usize],
    pub reserved1: mfxU16,
    pub MfxNalUnitType: mfxU16,
    pub SkipFrame: mfxU16,
    pub QP: mfxU16,
    pub FrameType: mfxU16,
    pub NumExtParam: mfxU16,
    pub NumPayload: mfxU16,
    pub reserved2: mfxU16,
    pub ExtParam: *mut *mut mfxExtBuffer,
    pub Payload: *mut *mut mfxPayload,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of mfxEncodeCtrl"][::core::mem::size_of::<mfxEncodeCtrl>() - 56usize];
    ["Alignment of mfxEncodeCtrl"][::core::mem::align_of::<mfxEncodeCtrl>() - 8usize];
    ["Offset of field: mfxEncodeCtrl::Header"][::core::mem::offset_of!(mfxEncodeCtrl, Header) - 0usize];
    ["Offset of field: mfxEncodeCtrl::reserved"][::core::mem::offset_of!(mfxEncodeCtrl, reserved) - 8usize];
    ["Offset of field: mfxEncodeCtrl::reserved1"][::core::mem::offset_of!(mfxEncodeCtrl, reserved1) - 24usize];
    ["Offset of field: mfxEncodeCtrl::MfxNalUnitType"]
        [::core::mem::offset_of!(mfxEncodeCtrl, MfxNalUnitType) - 26usize];
    ["Offset of field: mfxEncodeCtrl::SkipFrame"][::core::mem::offset_of!(mfxEncodeCtrl, SkipFrame) - 28usize];
    ["Offset of field: mfxEncodeCtrl::QP"][::core::mem::offset_of!(mfxEncodeCtrl, QP) - 30usize];
    ["Offset of field: mfxEncodeCtrl::FrameType"][::core::mem::offset_of!(mfxEncodeCtrl, FrameType) - 32usize];
    ["Offset of field: mfxEncodeCtrl::NumExtParam"][::core::mem::offset_of!(mfxEncodeCtrl, NumExtParam) - 34usize];
    ["Offset of field: mfxEncodeCtrl::NumPayload"][::core::mem::offset_of!(mfxEncodeCtrl, NumPayload) - 36usize];
    ["Offset of field: mfxEncodeCtrl::reserved2"][::core::mem::offset_of!(mfxEncodeCtrl, reserved2) - 38usize];
    ["Offset of field: mfxEncodeCtrl::ExtParam"][::core::mem::offset_of!(mfxEncodeCtrl, ExtParam) - 40usize];
    ["Offset of field: mfxEncodeCtrl::Payload"][::core::mem::offset_of!(mfxEncodeCtrl, Payload) - 48usize];
};
pub const MFX_MEMTYPE_PERSISTENT_MEMORY: _bindgen_ty_35 = 2;
pub type _bindgen_ty_35 = ::core::ffi::c_int;
pub const MFX_MEMTYPE_DXVA2_DECODER_TARGET: _bindgen_ty_36 = 16;
pub const MFX_MEMTYPE_DXVA2_PROCESSOR_TARGET: _bindgen_ty_36 = 32;
pub const MFX_MEMTYPE_VIDEO_MEMORY_DECODER_TARGET: _bindgen_ty_36 = 16;
pub const MFX_MEMTYPE_VIDEO_MEMORY_PROCESSOR_TARGET: _bindgen_ty_36 = 32;
pub const MFX_MEMTYPE_SYSTEM_MEMORY: _bindgen_ty_36 = 64;
pub const MFX_MEMTYPE_RESERVED1: _bindgen_ty_36 = 128;
pub const MFX_MEMTYPE_FROM_ENCODE: _bindgen_ty_36 = 256;
pub const MFX_MEMTYPE_FROM_DECODE: _bindgen_ty_36 = 512;
pub const MFX_MEMTYPE_FROM_VPPIN: _bindgen_ty_36 = 1024;
pub const MFX_MEMTYPE_FROM_VPPOUT: _bindgen_ty_36 = 2048;
pub const MFX_MEMTYPE_FROM_ENC: _bindgen_ty_36 = 8192;
pub const MFX_MEMTYPE_FROM_PAK: _bindgen_ty_36 = 16384;
pub const MFX_MEMTYPE_INTERNAL_FRAME: _bindgen_ty_36 = 1;
pub const MFX_MEMTYPE_EXTERNAL_FRAME: _bindgen_ty_36 = 2;
pub const MFX_MEMTYPE_EXPORT_FRAME: _bindgen_ty_36 = 8;
pub const MFX_MEMTYPE_SHARED_RESOURCE: _bindgen_ty_36 = 8;
pub const MFX_MEMTYPE_VIDEO_MEMORY_ENCODER_TARGET: _bindgen_ty_36 = 4096;
pub const MFX_MEMTYPE_VIDEO_MEMORY_UNORDERED_ACCESS: _bindgen_ty_36 = 32768;
pub type _bindgen_ty_36 = ::core::ffi::c_int;
#[repr(C)]
#[derive(Copy, Clone)]
pub struct mfxFrameAllocRequest {
    pub __bindgen_anon_1: mfxFrameAllocRequest__bindgen_ty_1,
    pub reserved3: [mfxU32; 3usize],
    pub Info: mfxFrameInfo,
    pub Type: mfxU16,
    pub NumFrameMin: mfxU16,
    pub NumFrameSuggested: mfxU16,
    pub reserved2: mfxU16,
}
#[repr(C)]
#[derive(Copy, Clone)]
pub union mfxFrameAllocRequest__bindgen_ty_1 {
    pub AllocId: mfxU32,
    pub reserved: [mfxU32; 1usize],
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of mfxFrameAllocRequest__bindgen_ty_1"]
        [::core::mem::size_of::<mfxFrameAllocRequest__bindgen_ty_1>() - 4usize];
    ["Alignment of mfxFrameAllocRequest__bindgen_ty_1"]
        [::core::mem::align_of::<mfxFrameAllocRequest__bindgen_ty_1>() - 4usize];
    ["Offset of field: mfxFrameAllocRequest__bindgen_ty_1::AllocId"]
        [::core::mem::offset_of!(mfxFrameAllocRequest__bindgen_ty_1, AllocId) - 0usize];
    ["Offset of field: mfxFrameAllocRequest__bindgen_ty_1::reserved"]
        [::core::mem::offset_of!(mfxFrameAllocRequest__bindgen_ty_1, reserved) - 0usize];
};
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of mfxFrameAllocRequest"][::core::mem::size_of::<mfxFrameAllocRequest>() - 92usize];
    ["Alignment of mfxFrameAllocRequest"][::core::mem::align_of::<mfxFrameAllocRequest>() - 4usize];
    ["Offset of field: mfxFrameAllocRequest::reserved3"]
        [::core::mem::offset_of!(mfxFrameAllocRequest, reserved3) - 4usize];
    ["Offset of field: mfxFrameAllocRequest::Info"][::core::mem::offset_of!(mfxFrameAllocRequest, Info) - 16usize];
    ["Offset of field: mfxFrameAllocRequest::Type"][::core::mem::offset_of!(mfxFrameAllocRequest, Type) - 84usize];
    ["Offset of field: mfxFrameAllocRequest::NumFrameMin"]
        [::core::mem::offset_of!(mfxFrameAllocRequest, NumFrameMin) - 86usize];
    ["Offset of field: mfxFrameAllocRequest::NumFrameSuggested"]
        [::core::mem::offset_of!(mfxFrameAllocRequest, NumFrameSuggested) - 88usize];
    ["Offset of field: mfxFrameAllocRequest::reserved2"]
        [::core::mem::offset_of!(mfxFrameAllocRequest, reserved2) - 90usize];
};
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct mfxFrameAllocResponse {
    pub AllocId: mfxU32,
    pub reserved: [mfxU32; 3usize],
    pub mids: *mut mfxMemId,
    pub NumFrameActual: mfxU16,
    pub reserved2: mfxU16,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of mfxFrameAllocResponse"][::core::mem::size_of::<mfxFrameAllocResponse>() - 32usize];
    ["Alignment of mfxFrameAllocResponse"][::core::mem::align_of::<mfxFrameAllocResponse>() - 8usize];
    ["Offset of field: mfxFrameAllocResponse::AllocId"]
        [::core::mem::offset_of!(mfxFrameAllocResponse, AllocId) - 0usize];
    ["Offset of field: mfxFrameAllocResponse::reserved"]
        [::core::mem::offset_of!(mfxFrameAllocResponse, reserved) - 4usize];
    ["Offset of field: mfxFrameAllocResponse::mids"][::core::mem::offset_of!(mfxFrameAllocResponse, mids) - 16usize];
    ["Offset of field: mfxFrameAllocResponse::NumFrameActual"]
        [::core::mem::offset_of!(mfxFrameAllocResponse, NumFrameActual) - 24usize];
    ["Offset of field: mfxFrameAllocResponse::reserved2"]
        [::core::mem::offset_of!(mfxFrameAllocResponse, reserved2) - 26usize];
};
pub const MFX_FRAMETYPE_UNKNOWN: _bindgen_ty_37 = 0;
pub const MFX_FRAMETYPE_I: _bindgen_ty_37 = 1;
pub const MFX_FRAMETYPE_P: _bindgen_ty_37 = 2;
pub const MFX_FRAMETYPE_B: _bindgen_ty_37 = 4;
pub const MFX_FRAMETYPE_S: _bindgen_ty_37 = 8;
pub const MFX_FRAMETYPE_REF: _bindgen_ty_37 = 64;
pub const MFX_FRAMETYPE_IDR: _bindgen_ty_37 = 128;
pub const MFX_FRAMETYPE_xI: _bindgen_ty_37 = 256;
pub const MFX_FRAMETYPE_xP: _bindgen_ty_37 = 512;
pub const MFX_FRAMETYPE_xB: _bindgen_ty_37 = 1024;
pub const MFX_FRAMETYPE_xS: _bindgen_ty_37 = 2048;
pub const MFX_FRAMETYPE_xREF: _bindgen_ty_37 = 16384;
pub const MFX_FRAMETYPE_xIDR: _bindgen_ty_37 = 32768;
pub type _bindgen_ty_37 = ::core::ffi::c_int;
pub const MFX_HEVC_NALU_TYPE_UNKNOWN: _bindgen_ty_38 = 0;
pub const MFX_HEVC_NALU_TYPE_TRAIL_N: _bindgen_ty_38 = 1;
pub const MFX_HEVC_NALU_TYPE_TRAIL_R: _bindgen_ty_38 = 2;
pub const MFX_HEVC_NALU_TYPE_RADL_N: _bindgen_ty_38 = 7;
pub const MFX_HEVC_NALU_TYPE_RADL_R: _bindgen_ty_38 = 8;
pub const MFX_HEVC_NALU_TYPE_RASL_N: _bindgen_ty_38 = 9;
pub const MFX_HEVC_NALU_TYPE_RASL_R: _bindgen_ty_38 = 10;
pub const MFX_HEVC_NALU_TYPE_IDR_W_RADL: _bindgen_ty_38 = 20;
pub const MFX_HEVC_NALU_TYPE_IDR_N_LP: _bindgen_ty_38 = 21;
pub const MFX_HEVC_NALU_TYPE_CRA_NUT: _bindgen_ty_38 = 22;
pub type _bindgen_ty_38 = ::core::ffi::c_int;
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct mfxExtVideoSignalInfo {
    pub Header: mfxExtBuffer,
    pub VideoFormat: mfxU16,
    pub VideoFullRange: mfxU16,
    pub ColourDescriptionPresent: mfxU16,
    pub ColourPrimaries: mfxU16,
    pub TransferCharacteristics: mfxU16,
    pub MatrixCoefficients: mfxU16,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of mfxExtVideoSignalInfo"][::core::mem::size_of::<mfxExtVideoSignalInfo>() - 20usize];
    ["Alignment of mfxExtVideoSignalInfo"][::core::mem::align_of::<mfxExtVideoSignalInfo>() - 4usize];
    ["Offset of field: mfxExtVideoSignalInfo::Header"][::core::mem::offset_of!(mfxExtVideoSignalInfo, Header) - 0usize];
    ["Offset of field: mfxExtVideoSignalInfo::VideoFormat"]
        [::core::mem::offset_of!(mfxExtVideoSignalInfo, VideoFormat) - 8usize];
    ["Offset of field: mfxExtVideoSignalInfo::VideoFullRange"]
        [::core::mem::offset_of!(mfxExtVideoSignalInfo, VideoFullRange) - 10usize];
    ["Offset of field: mfxExtVideoSignalInfo::ColourDescriptionPresent"]
        [::core::mem::offset_of!(mfxExtVideoSignalInfo, ColourDescriptionPresent) - 12usize];
    ["Offset of field: mfxExtVideoSignalInfo::ColourPrimaries"]
        [::core::mem::offset_of!(mfxExtVideoSignalInfo, ColourPrimaries) - 14usize];
    ["Offset of field: mfxExtVideoSignalInfo::TransferCharacteristics"]
        [::core::mem::offset_of!(mfxExtVideoSignalInfo, TransferCharacteristics) - 16usize];
    ["Offset of field: mfxExtVideoSignalInfo::MatrixCoefficients"]
        [::core::mem::offset_of!(mfxExtVideoSignalInfo, MatrixCoefficients) - 18usize];
};
pub const MFX_FRCALGM_PRESERVE_TIMESTAMP: _bindgen_ty_39 = 1;
pub const MFX_FRCALGM_DISTRIBUTED_TIMESTAMP: _bindgen_ty_39 = 2;
pub const MFX_FRCALGM_FRAME_INTERPOLATION: _bindgen_ty_39 = 4;
pub type _bindgen_ty_39 = ::core::ffi::c_int;
pub const MFX_IMAGESTAB_MODE_UPSCALE: _bindgen_ty_40 = 1;
pub const MFX_IMAGESTAB_MODE_BOXING: _bindgen_ty_40 = 2;
pub type _bindgen_ty_40 = ::core::ffi::c_int;
pub const MFX_PAYLOAD_OFF: _bindgen_ty_41 = 0;
pub const MFX_PAYLOAD_IDR: _bindgen_ty_41 = 1;
pub type _bindgen_ty_41 = ::core::ffi::c_int;
pub const MFX_LONGTERM_IDX_NO_IDX: _bindgen_ty_42 = 65535;
pub type _bindgen_ty_42 = ::core::ffi::c_int;
pub const MFX_TRANSFERMATRIX_UNKNOWN: _bindgen_ty_43 = 0;
pub const MFX_TRANSFERMATRIX_BT709: _bindgen_ty_43 = 1;
pub const MFX_TRANSFERMATRIX_BT601: _bindgen_ty_43 = 2;
pub type _bindgen_ty_43 = ::core::ffi::c_int;
pub const MFX_NOMINALRANGE_UNKNOWN: _bindgen_ty_44 = 0;
pub const MFX_NOMINALRANGE_0_255: _bindgen_ty_44 = 1;
pub const MFX_NOMINALRANGE_16_235: _bindgen_ty_44 = 2;
pub type _bindgen_ty_44 = ::core::ffi::c_int;
pub const MFX_ROI_MODE_PRIORITY: _bindgen_ty_45 = 0;
pub const MFX_ROI_MODE_QP_DELTA: _bindgen_ty_45 = 1;
pub const MFX_ROI_MODE_QP_VALUE: _bindgen_ty_45 = 2;
pub type _bindgen_ty_45 = ::core::ffi::c_int;
pub const MFX_DEINTERLACING_BOB: _bindgen_ty_46 = 1;
pub const MFX_DEINTERLACING_ADVANCED: _bindgen_ty_46 = 2;
pub const MFX_DEINTERLACING_AUTO_DOUBLE: _bindgen_ty_46 = 3;
pub const MFX_DEINTERLACING_AUTO_SINGLE: _bindgen_ty_46 = 4;
pub const MFX_DEINTERLACING_FULL_FR_OUT: _bindgen_ty_46 = 5;
pub const MFX_DEINTERLACING_HALF_FR_OUT: _bindgen_ty_46 = 6;
pub const MFX_DEINTERLACING_24FPS_OUT: _bindgen_ty_46 = 7;
pub const MFX_DEINTERLACING_FIXED_TELECINE_PATTERN: _bindgen_ty_46 = 8;
pub const MFX_DEINTERLACING_30FPS_OUT: _bindgen_ty_46 = 9;
pub const MFX_DEINTERLACING_DETECT_INTERLACE: _bindgen_ty_46 = 10;
pub const MFX_DEINTERLACING_ADVANCED_NOREF: _bindgen_ty_46 = 11;
pub const MFX_DEINTERLACING_ADVANCED_SCD: _bindgen_ty_46 = 12;
pub const MFX_DEINTERLACING_FIELD_WEAVING: _bindgen_ty_46 = 13;
pub type _bindgen_ty_46 = ::core::ffi::c_int;
pub const MFX_TELECINE_PATTERN_32: _bindgen_ty_47 = 0;
pub const MFX_TELECINE_PATTERN_2332: _bindgen_ty_47 = 1;
pub const MFX_TELECINE_PATTERN_FRAME_REPEAT: _bindgen_ty_47 = 2;
pub const MFX_TELECINE_PATTERN_41: _bindgen_ty_47 = 3;
pub const MFX_TELECINE_POSITION_PROVIDED: _bindgen_ty_47 = 4;
pub type _bindgen_ty_47 = ::core::ffi::c_int;
pub const MFX_VPP_COPY_FRAME: _bindgen_ty_48 = 1;
pub const MFX_VPP_COPY_FIELD: _bindgen_ty_48 = 2;
pub const MFX_VPP_SWAP_FIELDS: _bindgen_ty_48 = 3;
pub type _bindgen_ty_48 = ::core::ffi::c_int;
pub const MFX_PICTYPE_UNKNOWN: _bindgen_ty_49 = 0;
pub const MFX_PICTYPE_FRAME: _bindgen_ty_49 = 1;
pub const MFX_PICTYPE_TOPFIELD: _bindgen_ty_49 = 2;
pub const MFX_PICTYPE_BOTTOMFIELD: _bindgen_ty_49 = 4;
pub type _bindgen_ty_49 = ::core::ffi::c_int;
pub const MFX_MBQP_MODE_QP_VALUE: _bindgen_ty_50 = 0;
pub const MFX_MBQP_MODE_QP_DELTA: _bindgen_ty_50 = 1;
pub const MFX_MBQP_MODE_QP_ADAPTIVE: _bindgen_ty_50 = 2;
pub type _bindgen_ty_50 = ::core::ffi::c_int;
pub const MFX_HEVC_CONSTR_REXT_MAX_12BIT: _bindgen_ty_51 = 1;
pub const MFX_HEVC_CONSTR_REXT_MAX_10BIT: _bindgen_ty_51 = 2;
pub const MFX_HEVC_CONSTR_REXT_MAX_8BIT: _bindgen_ty_51 = 4;
pub const MFX_HEVC_CONSTR_REXT_MAX_422CHROMA: _bindgen_ty_51 = 8;
pub const MFX_HEVC_CONSTR_REXT_MAX_420CHROMA: _bindgen_ty_51 = 16;
pub const MFX_HEVC_CONSTR_REXT_MAX_MONOCHROME: _bindgen_ty_51 = 32;
pub const MFX_HEVC_CONSTR_REXT_INTRA: _bindgen_ty_51 = 64;
pub const MFX_HEVC_CONSTR_REXT_ONE_PICTURE_ONLY: _bindgen_ty_51 = 128;
pub const MFX_HEVC_CONSTR_REXT_LOWER_BIT_RATE: _bindgen_ty_51 = 256;
pub type _bindgen_ty_51 = ::core::ffi::c_int;
pub const MFX_SAO_UNKNOWN: _bindgen_ty_52 = 0;
pub const MFX_SAO_DISABLE: _bindgen_ty_52 = 1;
pub const MFX_SAO_ENABLE_LUMA: _bindgen_ty_52 = 2;
pub const MFX_SAO_ENABLE_CHROMA: _bindgen_ty_52 = 4;
pub type _bindgen_ty_52 = ::core::ffi::c_int;
pub const MFX_ERROR_NO: _bindgen_ty_53 = 0;
pub const MFX_ERROR_PPS: _bindgen_ty_53 = 1;
pub const MFX_ERROR_SPS: _bindgen_ty_53 = 2;
pub const MFX_ERROR_SLICEHEADER: _bindgen_ty_53 = 4;
pub const MFX_ERROR_SLICEDATA: _bindgen_ty_53 = 8;
pub const MFX_ERROR_FRAME_GAP: _bindgen_ty_53 = 16;
pub const MFX_ERROR_JPEG_APP0_MARKER: _bindgen_ty_53 = 32;
pub const MFX_ERROR_JPEG_APP1_MARKER: _bindgen_ty_53 = 64;
pub const MFX_ERROR_JPEG_APP2_MARKER: _bindgen_ty_53 = 128;
pub const MFX_ERROR_JPEG_APP3_MARKER: _bindgen_ty_53 = 256;
pub const MFX_ERROR_JPEG_APP4_MARKER: _bindgen_ty_53 = 512;
pub const MFX_ERROR_JPEG_APP5_MARKER: _bindgen_ty_53 = 1024;
pub const MFX_ERROR_JPEG_APP6_MARKER: _bindgen_ty_53 = 2048;
pub const MFX_ERROR_JPEG_APP7_MARKER: _bindgen_ty_53 = 4096;
pub const MFX_ERROR_JPEG_APP8_MARKER: _bindgen_ty_53 = 8192;
pub const MFX_ERROR_JPEG_APP9_MARKER: _bindgen_ty_53 = 16384;
pub const MFX_ERROR_JPEG_APP10_MARKER: _bindgen_ty_53 = 32768;
pub const MFX_ERROR_JPEG_APP11_MARKER: _bindgen_ty_53 = 65536;
pub const MFX_ERROR_JPEG_APP12_MARKER: _bindgen_ty_53 = 131072;
pub const MFX_ERROR_JPEG_APP13_MARKER: _bindgen_ty_53 = 262144;
pub const MFX_ERROR_JPEG_APP14_MARKER: _bindgen_ty_53 = 524288;
pub const MFX_ERROR_JPEG_DQT_MARKER: _bindgen_ty_53 = 1048576;
pub const MFX_ERROR_JPEG_SOF0_MARKER: _bindgen_ty_53 = 2097152;
pub const MFX_ERROR_JPEG_DHT_MARKER: _bindgen_ty_53 = 4194304;
pub const MFX_ERROR_JPEG_DRI_MARKER: _bindgen_ty_53 = 8388608;
pub const MFX_ERROR_JPEG_SOS_MARKER: _bindgen_ty_53 = 16777216;
pub const MFX_ERROR_JPEG_UNKNOWN_MARKER: _bindgen_ty_53 = 33554432;
pub type _bindgen_ty_53 = ::core::ffi::c_int;
pub const MFX_HEVC_REGION_SLICE: _bindgen_ty_54 = 0;
pub type _bindgen_ty_54 = ::core::ffi::c_int;
pub const MFX_HEVC_REGION_ENCODING_ON: _bindgen_ty_55 = 0;
pub const MFX_HEVC_REGION_ENCODING_OFF: _bindgen_ty_55 = 1;
pub type _bindgen_ty_55 = ::core::ffi::c_int;
pub const MFX_ANGLE_0: _bindgen_ty_56 = 0;
pub const MFX_ANGLE_90: _bindgen_ty_56 = 90;
pub const MFX_ANGLE_180: _bindgen_ty_56 = 180;
pub const MFX_ANGLE_270: _bindgen_ty_56 = 270;
pub type _bindgen_ty_56 = ::core::ffi::c_int;
pub const MFX_SCALING_MODE_DEFAULT: _bindgen_ty_57 = 0;
pub const MFX_SCALING_MODE_LOWPOWER: _bindgen_ty_57 = 1;
pub const MFX_SCALING_MODE_QUALITY: _bindgen_ty_57 = 2;
pub const MFX_SCALING_MODE_VENDOR: _bindgen_ty_57 = 1000;
pub const MFX_SCALING_MODE_INTEL_GEN_COMPUTE: _bindgen_ty_57 = 1001;
pub const MFX_SCALING_MODE_INTEL_GEN_VDBOX: _bindgen_ty_57 = 1002;
pub const MFX_SCALING_MODE_INTEL_GEN_VEBOX: _bindgen_ty_57 = 1003;
pub type _bindgen_ty_57 = ::core::ffi::c_int;
pub const MFX_INTERPOLATION_DEFAULT: _bindgen_ty_58 = 0;
pub const MFX_INTERPOLATION_NEAREST_NEIGHBOR: _bindgen_ty_58 = 1;
pub const MFX_INTERPOLATION_BILINEAR: _bindgen_ty_58 = 2;
pub const MFX_INTERPOLATION_ADVANCED: _bindgen_ty_58 = 3;
pub type _bindgen_ty_58 = ::core::ffi::c_int;
pub const MFX_MIRRORING_DISABLED: _bindgen_ty_59 = 0;
pub const MFX_MIRRORING_HORIZONTAL: _bindgen_ty_59 = 1;
pub const MFX_MIRRORING_VERTICAL: _bindgen_ty_59 = 2;
pub type _bindgen_ty_59 = ::core::ffi::c_int;
pub const MFX_CHROMA_SITING_UNKNOWN: _bindgen_ty_60 = 0;
pub const MFX_CHROMA_SITING_VERTICAL_TOP: _bindgen_ty_60 = 1;
pub const MFX_CHROMA_SITING_VERTICAL_CENTER: _bindgen_ty_60 = 2;
pub const MFX_CHROMA_SITING_VERTICAL_BOTTOM: _bindgen_ty_60 = 4;
pub const MFX_CHROMA_SITING_HORIZONTAL_LEFT: _bindgen_ty_60 = 16;
pub const MFX_CHROMA_SITING_HORIZONTAL_CENTER: _bindgen_ty_60 = 32;
pub type _bindgen_ty_60 = ::core::ffi::c_int;
pub const MFX_VP9_REF_INTRA: _bindgen_ty_61 = 0;
pub const MFX_VP9_REF_LAST: _bindgen_ty_61 = 1;
pub const MFX_VP9_REF_GOLDEN: _bindgen_ty_61 = 2;
pub const MFX_VP9_REF_ALTREF: _bindgen_ty_61 = 3;
pub type _bindgen_ty_61 = ::core::ffi::c_int;
pub const MFX_VP9_SEGMENT_ID_BLOCK_SIZE_UNKNOWN: _bindgen_ty_62 = 0;
pub const MFX_VP9_SEGMENT_ID_BLOCK_SIZE_8x8: _bindgen_ty_62 = 8;
pub const MFX_VP9_SEGMENT_ID_BLOCK_SIZE_16x16: _bindgen_ty_62 = 16;
pub const MFX_VP9_SEGMENT_ID_BLOCK_SIZE_32x32: _bindgen_ty_62 = 32;
pub const MFX_VP9_SEGMENT_ID_BLOCK_SIZE_64x64: _bindgen_ty_62 = 64;
pub type _bindgen_ty_62 = ::core::ffi::c_int;
pub const MFX_VP9_SEGMENT_FEATURE_QINDEX: _bindgen_ty_63 = 1;
pub const MFX_VP9_SEGMENT_FEATURE_LOOP_FILTER: _bindgen_ty_63 = 2;
pub const MFX_VP9_SEGMENT_FEATURE_REFERENCE: _bindgen_ty_63 = 4;
pub const MFX_VP9_SEGMENT_FEATURE_SKIP: _bindgen_ty_63 = 8;
pub type _bindgen_ty_63 = ::core::ffi::c_int;
pub const MFX_PARTIAL_BITSTREAM_NONE: _bindgen_ty_64 = 0;
pub const MFX_PARTIAL_BITSTREAM_SLICE: _bindgen_ty_64 = 1;
pub const MFX_PARTIAL_BITSTREAM_BLOCK: _bindgen_ty_64 = 2;
pub const MFX_PARTIAL_BITSTREAM_ANY: _bindgen_ty_64 = 3;
pub type _bindgen_ty_64 = ::core::ffi::c_int;
pub const MFX_AV1_SEGMENT_FEATURE_ALT_QINDEX: _bindgen_ty_65 = 1;
pub const MFX_AV1_SEGMENT_FEATURE_ALT_LF_Y_VERT: _bindgen_ty_65 = 2;
pub const MFX_AV1_SEGMENT_FEATURE_ALT_LF_Y_HORZ: _bindgen_ty_65 = 4;
pub const MFX_AV1_SEGMENT_FEATURE_ALT_LF_U: _bindgen_ty_65 = 8;
pub const MFX_AV1_SEGMENT_FEATURE_ALT_LF_V: _bindgen_ty_65 = 16;
pub const MFX_AV1_SEGMENT_FEATURE_REFERENCE: _bindgen_ty_65 = 32;
pub const MFX_AV1_SEGMENT_FEATURE_SKIP: _bindgen_ty_65 = 64;
pub const MFX_AV1_SEGMENT_FEATURE_GLOBALMV: _bindgen_ty_65 = 128;
pub type _bindgen_ty_65 = ::core::ffi::c_int;
pub const MFX_FILM_GRAIN_NO: _bindgen_ty_66 = 0;
pub const MFX_FILM_GRAIN_APPLY: _bindgen_ty_66 = 1;
pub const MFX_FILM_GRAIN_UPDATE: _bindgen_ty_66 = 2;
pub const MFX_FILM_GRAIN_CHROMA_SCALING_FROM_LUMA: _bindgen_ty_66 = 4;
pub const MFX_FILM_GRAIN_OVERLAP: _bindgen_ty_66 = 8;
pub const MFX_FILM_GRAIN_CLIP_TO_RESTRICTED_RANGE: _bindgen_ty_66 = 16;
pub type _bindgen_ty_66 = ::core::ffi::c_int;
pub const MFX_ALPHA_MODE_PREMULTIPLIED: _bindgen_ty_67 = 1;
pub const MFX_ALPHA_MODE_STRAIGHT: _bindgen_ty_67 = 2;
pub type _bindgen_ty_67 = ::core::ffi::c_int;
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct mfxFrameAllocator {
    pub reserved: [mfxU32; 4usize],
    pub pthis: mfxHDL,
    pub Alloc: ::core::option::Option<
        unsafe extern "C" fn(
            pthis: mfxHDL,
            request: *mut mfxFrameAllocRequest,
            response: *mut mfxFrameAllocResponse,
        ) -> mfxStatus,
    >,
    pub Lock:
        ::core::option::Option<unsafe extern "C" fn(pthis: mfxHDL, mid: mfxMemId, ptr: *mut mfxFrameData) -> mfxStatus>,
    pub Unlock:
        ::core::option::Option<unsafe extern "C" fn(pthis: mfxHDL, mid: mfxMemId, ptr: *mut mfxFrameData) -> mfxStatus>,
    pub GetHDL:
        ::core::option::Option<unsafe extern "C" fn(pthis: mfxHDL, mid: mfxMemId, handle: *mut mfxHDL) -> mfxStatus>,
    pub Free:
        ::core::option::Option<unsafe extern "C" fn(pthis: mfxHDL, response: *mut mfxFrameAllocResponse) -> mfxStatus>,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of mfxFrameAllocator"][::core::mem::size_of::<mfxFrameAllocator>() - 64usize];
    ["Alignment of mfxFrameAllocator"][::core::mem::align_of::<mfxFrameAllocator>() - 8usize];
    ["Offset of field: mfxFrameAllocator::reserved"][::core::mem::offset_of!(mfxFrameAllocator, reserved) - 0usize];
    ["Offset of field: mfxFrameAllocator::pthis"][::core::mem::offset_of!(mfxFrameAllocator, pthis) - 16usize];
    ["Offset of field: mfxFrameAllocator::Alloc"][::core::mem::offset_of!(mfxFrameAllocator, Alloc) - 24usize];
    ["Offset of field: mfxFrameAllocator::Lock"][::core::mem::offset_of!(mfxFrameAllocator, Lock) - 32usize];
    ["Offset of field: mfxFrameAllocator::Unlock"][::core::mem::offset_of!(mfxFrameAllocator, Unlock) - 40usize];
    ["Offset of field: mfxFrameAllocator::GetHDL"][::core::mem::offset_of!(mfxFrameAllocator, GetHDL) - 48usize];
    ["Offset of field: mfxFrameAllocator::Free"][::core::mem::offset_of!(mfxFrameAllocator, Free) - 56usize];
};
#[repr(C)]
#[derive(Debug)]
pub struct _mfxLoader {
    _unused: [u8; 0],
}
pub type mfxLoader = *mut _mfxLoader;
#[repr(C)]
#[derive(Debug)]
pub struct _mfxConfig {
    _unused: [u8; 0],
}
pub type mfxConfig = *mut _mfxConfig;

// ---- Вручную: сигнатуры функций диспетчера (mfxdispatcher.h, mfxsession.h, mfxvideo.h) ----

pub type MFXLoadFn = unsafe extern "C" fn() -> mfxLoader;
pub type MFXUnloadFn = unsafe extern "C" fn(mfxLoader);
pub type MFXCreateConfigFn = unsafe extern "C" fn(mfxLoader) -> mfxConfig;
pub type MFXSetConfigFilterPropertyFn = unsafe extern "C" fn(mfxConfig, *const mfxU8, mfxVariant) -> mfxStatus;
pub type MFXEnumImplementationsFn =
    unsafe extern "C" fn(mfxLoader, mfxU32, mfxImplCapsDeliveryFormat, *mut mfxHDL) -> mfxStatus;
pub type MFXDispReleaseImplDescriptionFn = unsafe extern "C" fn(mfxLoader, mfxHDL) -> mfxStatus;
pub type MFXCreateSessionFn = unsafe extern "C" fn(mfxLoader, mfxU32, *mut mfxSession) -> mfxStatus;
pub type MFXCloseFn = unsafe extern "C" fn(mfxSession) -> mfxStatus;
pub type MFXQueryVersionFn = unsafe extern "C" fn(mfxSession, *mut mfxVersion) -> mfxStatus;
pub type MFXVideoCORE_SetHandleFn = unsafe extern "C" fn(mfxSession, mfxHandleType, mfxHDL) -> mfxStatus;
pub type MFXVideoCORE_SetFrameAllocatorFn = unsafe extern "C" fn(mfxSession, *mut mfxFrameAllocator) -> mfxStatus;
pub type MFXVideoCORE_SyncOperationFn = unsafe extern "C" fn(mfxSession, mfxSyncPoint, mfxU32) -> mfxStatus;
pub type MFXVideoENCODE_QueryIOSurfFn =
    unsafe extern "C" fn(mfxSession, *mut mfxVideoParam, *mut mfxFrameAllocRequest) -> mfxStatus;
pub type MFXVideoENCODE_InitFn = unsafe extern "C" fn(mfxSession, *mut mfxVideoParam) -> mfxStatus;
pub type MFXVideoENCODE_CloseFn = unsafe extern "C" fn(mfxSession) -> mfxStatus;
pub type MFXVideoENCODE_GetVideoParamFn = unsafe extern "C" fn(mfxSession, *mut mfxVideoParam) -> mfxStatus;
pub type MFXVideoENCODE_EncodeFrameAsyncFn = unsafe extern "C" fn(
    mfxSession,
    *mut mfxEncodeCtrl,
    *mut mfxFrameSurface1,
    *mut mfxBitstream,
    *mut mfxSyncPoint,
) -> mfxStatus;

/// `MFX_STRUCT_VERSION(major, minor)`.
pub const fn struct_version(major: u8, minor: u8) -> mfxStructVersion {
    mfxStructVersion { Version: (major as u16) << 8 | minor as u16 }
}
