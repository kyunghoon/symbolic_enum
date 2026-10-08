//! Procedural macro crate for high-performance, dual-mode unit enum serialization.
//!
//! `SymbolicEnum` provides bidirectional string/numeric conversions, compile-time
//! hash generation, static iteration, and adaptive Serde serialization. Human-readable
//! formats (JSON, YAML, TOML) serialize to string variant names by default, while
//! binary formats (Bincode, Postcard, MessagePack) serialize to 32-bit FNV-1a hashes
//! for minimal wire size.

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields};

/// Calculates a 32-bit FNV-1a non-cryptographic hash for string identifiers at macro expansion time.
#[inline]
fn fnv1a_32(s: &str) -> u32 {
    let bytes = s.as_bytes();
    let mut hash = 0x811c9dc5u32;
    for &b in bytes {
        hash ^= b as u32;
        hash = hash.wrapping_mul(0x01000193);
    }
    hash
}

/// Derives `SymbolicEnum` for unit-only enums.
///
/// # Derived Inherent Methods
/// - `ALL_VARIANTS`: Static slice containing all variants in declaration order.
/// - `iter()`: Returns an iterator over `ALL_VARIANTS`.
/// - `hash_id()`: Returns the precomputed 32-bit FNV-1a hash (`u32`) for the variant.
/// - `from_hash(hash: u32)`: Resolves an enum variant from a 32-bit hash.
/// - `as_str()`: Returns the static string representation of the variant name.
/// - `from_str(s: &str)`: Resolves an enum variant from a string identifier.
///
/// # Derived Trait Implementations
/// - [`core::str::FromStr`]: Converts string names to variants.
/// - [`IntoIterator`]: Iterates over static variant references.
/// - [`serde::Serialize`]: Dual-mode serialization (human-readable string vs. binary u32).
/// - [`serde::Deserialize`]: Dual-mode deserialization with cross-type fallback parsing.
#[proc_macro_derive(SymbolicEnum)]
pub fn derive_symbolic_enum(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let enum_name = &input.ident;

    let data_enum = match &input.data {
        Data::Enum(e) => e,
        _ => {
            return syn::Error::new_spanned(
                enum_name,
                "SymbolicEnum can only be derived on enums",
            )
            .to_compile_error()
            .into();
        }
    };

    let mut variant_idents = Vec::with_capacity(data_enum.variants.len());
    let mut variant_strings = Vec::with_capacity(data_enum.variants.len());
    let mut variant_hashes = Vec::with_capacity(data_enum.variants.len());

    for variant in &data_enum.variants {
        if !matches!(variant.fields, Fields::Unit) {
            return syn::Error::new_spanned(
                &variant.ident,
                "SymbolicEnum only supports unit variants (e.g., `VariantName`)",
            )
            .to_compile_error()
            .into();
        }

        let ident = &variant.ident;
        let name_str = ident.to_string();
        let hash = fnv1a_32(&name_str);

        variant_idents.push(ident);
        variant_strings.push(name_str);
        variant_hashes.push(hash);
    }

    let serialize_human_readable = if cfg!(feature = "output_hash") {
        quote! { serializer.serialize_u32(self.hash_id()) }
    } else {
        quote! { serializer.serialize_str(self.as_str()) }
    };

    let expanded = quote! {
        impl #enum_name {
            #[doc = "Slice containing all enum variants in declaration order."]
            pub const ALL_VARIANTS: &'static [Self] = &[
                #( Self::#variant_idents, )*
            ];

            #[doc = "Returns an iterator over all static variants of the enum."]
            #[inline]
            pub fn iter() -> ::core::slice::Iter<'static, Self> {
                Self::ALL_VARIANTS.iter()
            }

            #[doc = "Returns the precomputed 32-bit FNV-1a hash discriminant for this variant."]
            #[inline]
            pub const fn hash_id(&self) -> ::core::primitive::u32 {
                match self {
                    #( Self::#variant_idents => #variant_hashes, )*
                }
            }

            #[doc = "Attempts to resolve an enum variant from a 32-bit FNV-1a hash discriminant."]
            #[inline]
            pub fn from_hash(hash: ::core::primitive::u32) -> ::core::option::Option<Self> {
                match hash {
                    #( #variant_hashes => ::core::option::Option::Some(Self::#variant_idents), )*
                    _ => ::core::option::Option::None,
                }
            }

            #[doc = "Returns the static string representation of this variant name."]
            #[inline]
            pub const fn as_str(&self) -> &'static ::core::primitive::str {
                match self {
                    #( Self::#variant_idents => #variant_strings, )*
                }
            }

            #[doc = "Attempts to resolve an enum variant from its string identifier."]
            #[inline]
            pub fn from_str(s: &::core::primitive::str) -> ::core::option::Option<Self> {
                match s {
                    #( #variant_strings => ::core::option::Option::Some(Self::#variant_idents), )*
                    _ => ::core::option::Option::None,
                }
            }
        }

        impl ::core::str::FromStr for #enum_name {
            type Err = ::std::string::String;

            fn from_str(s: &::core::primitive::str) -> ::core::result::Result<Self, Self::Err> {
                Self::from_str(s).ok_or_else(|| {
                    ::std::format!(
                        "Unknown variant string '{}' for enum '{}'",
                        s,
                        ::core::stringify!(#enum_name)
                    )
                })
            }
        }

        impl<'a> ::core::iter::IntoIterator for &'a #enum_name {
            type Item = &'static #enum_name;
            type IntoIter = ::core::slice::Iter<'static, #enum_name>;

            fn into_iter(self) -> Self::IntoIter {
                #enum_name::ALL_VARIANTS.iter()
            }
        }

        // Static compile-time assertion guarding against FNV-1a hash collisions
        const _: () = {
            let hashes = [#(#variant_hashes),*];
            let mut i = 0;
            while i < hashes.len() {
                let mut j = i + 1;
                while j < hashes.len() {
                    if hashes[i] == hashes[j] {
                        panic!(::core::concat!(
                            "FNV-1a hash collision detected during `SymbolicEnum` expansion for enum `",
                            ::core::stringify!(#enum_name),
                            "`"
                        ));
                    }
                    j += 1;
                }
                i += 1;
            }
        };

        impl ::serde::Serialize for #enum_name {
            fn serialize<S>(&self, serializer: S) -> ::core::result::Result<S::Ok, S::Error>
            where
                S: ::serde::Serializer,
            {
                if serializer.is_human_readable() {
                    #serialize_human_readable
                } else {
                    serializer.serialize_u32(self.hash_id())
                }
            }
        }

        impl<'de> ::serde::Deserialize<'de> for #enum_name {
            fn deserialize<D>(deserializer: D) -> ::core::result::Result<Self, D::Error>
            where
                D: ::serde::Deserializer<'de>,
            {
                struct EnumVisitor;

                impl<'de> ::serde::de::Visitor<'de> for EnumVisitor {
                    type Value = #enum_name;

                    fn expecting(&self, formatter: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
                        formatter.write_str(::core::concat!(
                            "a variant identifier string, a numeric hash string, or a 32-bit hash discriminant for `",
                            ::core::stringify!(#enum_name),
                            "`"
                        ))
                    }

                    #[inline]
                    fn visit_str<E>(self, v: &::core::primitive::str) -> ::core::result::Result<Self::Value, E>
                    where
                        E: ::serde::de::Error,
                    {
                        // Path 1: Variant string match (e.g. "TownSquare")
                        if let ::core::option::Option::Some(variant) = #enum_name::from_str(v) {
                            return ::core::result::Result::Ok(variant);
                        }

                        // Path 2: Fallback — Decimal u32 string (e.g. "2166136261")
                        if let ::core::result::Result::Ok(hash) = v.parse::<::core::primitive::u32>() {
                            if let ::core::option::Option::Some(variant) = #enum_name::from_hash(hash) {
                                return ::core::result::Result::Ok(variant);
                            }
                        }

                        // Path 3: Fallback — Hexadecimal u32 string (e.g. "0x811c9dc5")
                        if let ::core::option::Option::Some(hex_str) = v.strip_prefix("0x").or_else(|| v.strip_prefix("0X")) {
                            if let ::core::result::Result::Ok(hash) = ::core::primitive::u32::from_str_radix(hex_str, 16) {
                                if let ::core::option::Option::Some(variant) = #enum_name::from_hash(hash) {
                                    return ::core::result::Result::Ok(variant);
                                }
                            }
                        }

                        ::core::result::Result::Err(E::custom(::std::format!(
                            "unknown variant name or hash discriminant '{}' for enum `{}`",
                            v,
                            ::core::stringify!(#enum_name)
                        )))
                    }

                    #[inline]
                    fn visit_borrowed_str<E>(self, v: &'de ::core::primitive::str) -> ::core::result::Result<Self::Value, E>
                    where
                        E: ::serde::de::Error,
                    {
                        self.visit_str(v)
                    }

                    #[inline]
                    fn visit_string<E>(self, v: ::std::string::String) -> ::core::result::Result<Self::Value, E>
                    where
                        E: ::serde::de::Error,
                    {
                        self.visit_str(&v)
                    }

                    #[inline]
                    fn visit_u32<E>(self, v: ::core::primitive::u32) -> ::core::result::Result<Self::Value, E>
                    where
                        E: ::serde::de::Error,
                    {
                        #enum_name::from_hash(v).ok_or_else(|| {
                            E::custom(::std::format!(
                                "unknown 32-bit hash discriminant {} for enum `{}`",
                                v,
                                ::core::stringify!(#enum_name)
                            ))
                        })
                    }

                    #[inline]
                    fn visit_u64<E>(self, v: ::core::primitive::u64) -> ::core::result::Result<Self::Value, E>
                    where
                        E: ::serde::de::Error,
                    {
                        if let ::core::result::Result::Ok(v_u32) = ::core::convert::TryFrom::try_from(v) {
                            self.visit_u32(v_u32)
                        } else {
                            ::core::result::Result::Err(E::custom(::std::format!(
                                "numeric value {} exceeds u32 range for enum `{}`",
                                v,
                                ::core::stringify!(#enum_name)
                            )))
                        }
                    }

                    #[inline]
                    fn visit_i64<E>(self, v: ::core::primitive::i64) -> ::core::result::Result<Self::Value, E>
                    where
                        E: ::serde::de::Error,
                    {
                        if let ::core::result::Result::Ok(v_u32) = ::core::convert::TryFrom::try_from(v) {
                            self.visit_u32(v_u32)
                        } else {
                            ::core::result::Result::Err(E::custom(::std::format!(
                                "numeric value {} out of u32 bounds for enum `{}`",
                                v,
                                ::core::stringify!(#enum_name)
                            )))
                        }
                    }

                    #[inline]
                    fn visit_u8<E>(self, v: ::core::primitive::u8) -> ::core::result::Result<Self::Value, E>
                    where
                        E: ::serde::de::Error,
                    {
                        self.visit_u32(v as ::core::primitive::u32)
                    }

                    #[inline]
                    fn visit_u16<E>(self, v: ::core::primitive::u16) -> ::core::result::Result<Self::Value, E>
                    where
                        E: ::serde::de::Error,
                    {
                        self.visit_u32(v as ::core::primitive::u32)
                    }
                }

                if deserializer.is_human_readable() {
                    deserializer.deserialize_any(EnumVisitor)
                } else {
                    deserializer.deserialize_u32(EnumVisitor)
                }
            }
        }
    };

    TokenStream::from(expanded)
}