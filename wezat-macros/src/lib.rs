use proc_macro::TokenStream;

#[derive(Debug)]
struct Dependency {
    pub field: String,
    pub depends_on: String,
}

#[proc_macro_attribute]
pub fn wz(_attr: TokenStream, input: TokenStream) -> TokenStream {
    let mut input = syn::parse_macro_input!(input as syn::DeriveInput);
    let syn::Data::Struct(data) = &mut input.data else {
        panic!("not a struct");
    };

    let syn::Fields::Named(fields) = &mut data.fields else {
        panic!("expected named fields")
    };

    let original_named_fields = fields.named.iter().cloned().collect::<Vec<_>>();

    let mut dependencies = vec![];

    let mut fields_to_remove = vec![];

    for field in fields.named.iter_mut() {
        match &mut field.ty {
            syn::Type::Array(array) => {
                // make sure len is a path
                let syn::Expr::Path(expr_path) = array.len.clone() else {
                    continue;
                    /*
                    panic!(
                        r#"len of "{}" len is not a path (len: {len:#?})"#,
                        field
                            .ident
                            .as_ref()
                            .map(|v| v.to_string())
                            .unwrap_or("error".into()),
                        len = array.len
                    );
                    */
                };

                let segments = expr_path.path.segments;

                // more than one segment => not a field name
                if segments.len() != 1 {
                    continue;
                }

                let type_segment = &segments.first().unwrap().ident;

                let struct_is_using_variable_name = original_named_fields
                    .iter()
                    .filter_map(|original_field| {
                        let Some(ident) = &original_field.ident else {
                            return None;
                        };
                        Some(ident)
                    })
                    .any(|original_ident| original_ident == type_segment);

                // if it's not a known field in the struct, ignore it
                if !struct_is_using_variable_name {
                    panic!("struct not using {type_segment}");
                }

                {
                    let depends_on = type_segment.to_string();
                    let field = field.ident.as_ref().unwrap().to_string();

                    dependencies.push(Dependency { field, depends_on });
                }

                fields_to_remove.push(field.ident.clone());

                let elem = &array.elem;

                field.ty = syn::parse_quote! {
                    ::std::vec::Vec<#elem>
                };
            }
            syn::Type::Reference(type_reference) => {
                let syn::Type::Path(path) = type_reference.elem.as_ref() else {
                    continue;
                };

                let type_segment = {
                    if path.path.segments.len() != 1 {
                        continue;
                    }
                    path.path.segments.first().unwrap().ident.clone()
                };

                let struct_is_using_variable_name =
                    original_named_fields.iter().any(|original_field| {
                        let Some(original_ident) = &original_field.ident else {
                            return false;
                        };

                        *original_ident == type_segment
                    });

                // if it's not a known field in the struct, ignore it
                if !struct_is_using_variable_name {
                    panic!("struct not using {type_segment}");
                }

                // panic!("{type_reference:#?}");

                {
                    let depends_on = field.ident.as_ref().unwrap().to_string();
                    let field = type_segment.to_string();
                    dependencies.push(Dependency { field, depends_on });
                }

                fields_to_remove.push(field.ident.clone());

                let elem = &type_reference.elem;
                field.ty = syn::parse_quote! {
                    ::std::vec::Vec<#elem>
                };
            }
            _ => {}
        }
    }

    // go through identifiers in struct and get dependencies
    let filtered_named_fields = {
        let mut filtered = vec![];

        for named_field in &fields.named {
            // skip unidentifiable fields
            let Some(ident) = &named_field.ident else {
                continue;
            };

            if fields_to_remove.iter().any(|v| Some(ident) == v.as_ref()) {
                continue;
            }

            filtered.push(named_field.clone());
        }
        filtered.into_iter().collect()
    };

    for Dependency { field, depends_on } in dependencies {
        let field_index = fields
            .named
            .iter()
            .position(|v| Some(field.clone()) == v.ident.as_ref().map(|v| v.to_string()))
            .unwrap_or_else(|| panic!(r#"field "{field}" does not exist"#));

        let dep_index = fields
            .named
            .iter()
            .position(|v| Some(depends_on.clone()) == v.ident.as_ref().map(|v| v.to_string()))
            .unwrap_or_else(|| panic!(r#"field "{depends_on}" does not exist"#));

        if dep_index == field_index {
            panic!(r#"wezat error: field "{field}", depends on itself?"#);
        } else if field_index < dep_index {
            panic!(r#"`{field}` depends on `{depends_on}`, but `{field}` appears first."#);
        }
    }

    fields.named = filtered_named_fields;

    let field_idents = fields
        .named
        .iter()
        .filter_map(|v| v.ident.clone())
        .collect::<Vec<_>>();

    let struct_name = input.ident.clone();

    let quoted = quote::quote! {
        #input
        impl ::wezat::Wezat for #struct_name {
            const MIN_SIZE: usize = 0;

            fn from_bytes(reader: &mut impl ::wezat::Reader) -> Result<Self, ::wezat::Error> {
                #(
                    let #field_idents = ::wezat::Wezat::from_bytes(reader)?;
                )*

                Ok(Self {
                    #(
                        #field_idents,
                    )*
                })
            }

            fn write_bytes(&self, writer: &mut impl ::wezat::Writer) -> Result<(), ::wezat::Error> {
                todo!()
            }
        }
    };

    quoted.into()
}
