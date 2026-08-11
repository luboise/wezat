use proc_macro::TokenStream;

#[proc_macro_attribute]
pub fn wz(_attr: TokenStream, input: TokenStream) -> TokenStream {
    let mut input = syn::parse_macro_input!(input as syn::DeriveInput);
    let syn::Data::Struct(data) = &mut input.data else {
        panic!("not a struct");
    };

    let syn::Fields::Named(fields) = &mut data.fields else {
        panic!("expected named fields")
    };

    for field in fields.named.iter_mut() {
        if let syn::Type::Array(array) = &mut field.ty {
            let elem = &array.elem;
            field.ty = syn::parse_quote! {
                ::std::vec::Vec<#elem>
            };
        }
    }

    fields.named = fields
        .named
        .iter()
        .filter(|v| {
            !v.ident.as_ref().is_some_and(|v| {
                let str = v.to_string();
                str.ends_with("_len") || str.ends_with("_ptr")
            })
        })
        .cloned()
        .collect();

    let field_idents = fields
        .named
        .iter()
        .filter_map(|v| v.ident.clone())
        .collect::<Vec<_>>();

    let struct_name = input.ident.clone();

    quote::quote! {
        #input
        impl ::wezat::Wezat for #struct_name {
            const MIN_SIZE: usize = 0;

            fn from_bytes(reader: &mut impl ::wezat::Reader) -> Result<Self, ::wezat::Error> {
                Ok(Self {
                    #(
                        #field_idents: ::wezat::Wezat::from_bytes(reader)?,
                    )*
                })
            }

            fn write_bytes(&self, writer: &mut impl ::wezat::Writer) -> Result<(), ::wezat::Error> {
                todo!()
            }
        }
    }
    .into()
}
