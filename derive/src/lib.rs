use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields, Type};

/// Derive macro that generates legacy `encode_abci` plus an optional
/// coordinate-aware encoder. Passing `None` is byte-identical to the legacy
/// format; callers opt selected event families into four canonical chain
/// attributes without changing enum constructors.
/// for an enum where each variant has named fields.
///
/// Variant names are converted to snake_case event types.
/// Field types determine encoding:
///   - `u64`, `u32`, `i64` → decimal string
///   - `[u8; N]`            → hex string
///   - `Side`               → "buy"/"sell" via Display
///   - enums with Display   → `.to_string()`
///   - anything else with Display → `.to_string()`
///
/// Fields named with a leading underscore are skipped.
#[proc_macro_derive(AbciEvent)]
pub fn derive_abci_event(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    let variants = match &input.data {
        Data::Enum(data) => &data.variants,
        _ => panic!("AbciEvent can only be derived on enums"),
    };

    let arms: Vec<_> = variants
        .iter()
        .map(|variant| {
            let vname = &variant.ident;
            let event_type = to_snake_case(&vname.to_string());

            let fields = match &variant.fields {
                Fields::Named(f) => &f.named,
                _ => panic!("AbciEvent variants must have named fields"),
            };

            let field_names: Vec<_> = fields.iter().map(|f| f.ident.as_ref().unwrap()).collect();

            let attr_count = field_names.len() as u16;

            let write_stmts: Vec<_> = fields
                .iter()
                .map(|f| {
                    let fname = f.ident.as_ref().unwrap();
                    let key = fname.to_string();
                    let ty = &f.ty;

                    if is_option_byte_array(ty) {
                        // An optional address: `Some` hex-encodes like any
                        // byte array, `None` writes the key with an empty
                        // value. Empty is unambiguous — a real address is
                        // always 40 hex characters — and writing the
                        // attribute either way keeps the compile-time
                        // `attr_count` correct, which omitting it would not.
                        quote! {
                            match #fname {
                                Some(bytes) => w.write_attr_hex(#key, bytes),
                                None => w.write_attr(#key, ""),
                            }
                        }
                    } else if is_byte_array(ty) || is_vec_u8(ty) {
                        // Both fixed-size and variable-length byte sequences
                        // hex-encode for ABCI attribute output.
                        quote! { w.write_attr_hex(#key, #fname); }
                    } else if is_u64(ty) {
                        quote! { w.write_attr_u64(#key, *#fname); }
                    } else if is_u32(ty) {
                        quote! { w.write_attr_u64(#key, *#fname as u64); }
                    } else if is_i64(ty) {
                        // Signed integers: encode via Display to preserve the sign.
                        quote! { w.write_attr_display(#key, #fname); }
                    } else {
                        // Fallback: anything with Display (enums like Side, CancelReason)
                        quote! { w.write_attr_display(#key, #fname); }
                    }
                })
                .collect();

            quote! {
                #name::#vname { #(#field_names),* } => {
                    let coordinate_attrs = u16::from(coordinates.is_some()) * 4;
                    w.begin_event(#event_type, #attr_count + coordinate_attrs);
                    if let Some(coordinates) = coordinates {
                        coordinates.write_attrs(w);
                    }
                    #(#write_stmts)*
                }
            }
        })
        .collect();

    let expanded = quote! {
        impl #name {
            pub fn encode_abci(&self, w: &mut crate::abci_event::AbciEventWriter) {
                self.encode_abci_with_coordinates(w, None);
            }

            pub fn encode_abci_with_coordinates(
                &self,
                w: &mut crate::abci_event::AbciEventWriter,
                coordinates: Option<crate::abci_event::EventCoordinates>,
            ) {
                match self {
                    #(#arms)*
                }
            }
        }
    };

    expanded.into()
}

fn to_snake_case(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut result = String::with_capacity(s.len() + 4);
    for (i, &c) in chars.iter().enumerate() {
        if c.is_uppercase() {
            let prev_is_lower_or_digit =
                i > 0 && (chars[i - 1].is_ascii_lowercase() || chars[i - 1].is_ascii_digit());
            let next_is_lower = chars
                .get(i + 1)
                .map(|next| next.is_ascii_lowercase())
                .unwrap_or(false);
            if i > 0 && (prev_is_lower_or_digit || next_is_lower) {
                result.push('_');
            }
            result.push(c.to_ascii_lowercase());
        } else {
            result.push(c);
        }
    }
    result
}

fn is_byte_array(ty: &Type) -> bool {
    let s = quote!(#ty).to_string();
    s.starts_with("[u8 ;") || s.starts_with("[u8;")
}

/// `Option<[u8; N]>` — an address-shaped field that may be genuinely absent.
/// Kept distinct from [`is_byte_array`] so absence survives to the wire as an
/// empty attribute instead of being encoded as some in-band placeholder.
fn is_option_byte_array(ty: &Type) -> bool {
    let s = quote!(#ty).to_string().replace(' ', "");
    s.starts_with("Option<[u8;")
}

fn is_u64(ty: &Type) -> bool {
    matches_ident(ty, "u64")
}

fn is_u32(ty: &Type) -> bool {
    matches_ident(ty, "u32")
}

fn is_i64(ty: &Type) -> bool {
    matches_ident(ty, "i64")
}

fn is_vec_u8(ty: &Type) -> bool {
    let s = quote!(#ty).to_string();
    s == "Vec < u8 >" || s == "Vec<u8>"
}

fn matches_ident(ty: &Type, name: &str) -> bool {
    if let Type::Path(p) = ty {
        if let Some(seg) = p.path.segments.last() {
            return seg.ident == name;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::to_snake_case;

    #[test]
    fn snake_case_preserves_trailing_acronym() {
        assert_eq!(
            to_snake_case("BinaryIssuedFromMTM"),
            "binary_issued_from_mtm"
        );
    }

    #[test]
    fn snake_case_handles_normal_event_names() {
        assert_eq!(to_snake_case("OrderPlaced"), "order_placed");
        assert_eq!(to_snake_case("HlpAbsorbed"), "hlp_absorbed");
    }
}
