use proc_macro::TokenStream;

#[derive(Debug)]
struct Dependency {
    pub field: String,
    pub depends_on: String,
}

#[derive(Debug)]
enum Field {
    Normal {
        name: String,
    },
    Pointer {
        name: String,
        to_field: String,
    },
    Length {
        name: String,
        len: usize,
        for_field: String,
    },
}

impl Field {
    pub fn name(&self) -> &str {
        match self {
            Field::Normal { name } | Field::Pointer { name, .. } | Field::Length { name, .. } => {
                name.as_str()
            }
        }
    }
}

#[proc_macro_attribute]
pub fn wz(_attr: TokenStream, input: TokenStream) -> TokenStream {
    let mut input = syn::parse_macro_input!(input as syn::DeriveInput);
    let syn::Data::Struct(data) = &mut input.data else {
        panic!("not a struct");
    };

    // go through identifiers in struct and get dependencies
    let (parsed_fields, dependencies) = parse_fields(data).unwrap();

    let syn::Fields::Named(syn_fields) = &mut data.fields else {
        panic!("expected named fields")
    };

    for Dependency { field, depends_on } in dependencies {
        let field_index = parsed_fields
            .iter()
            .position(|f| f.name() == field)
            .unwrap_or_else(|| panic!(r#"field "{field}" does not exist"#));

        let dep_index = parsed_fields
            .iter()
            .position(|f| f.name() == depends_on)
            .unwrap_or_else(|| panic!(r#"field "{depends_on}" does not exist"#));

        if dep_index == field_index {
            panic!(r#"wezat error: field "{field}", depends on itself?"#);
        } else if field_index < dep_index {
            panic!(r#"`{field}` depends on `{depends_on}`, but `{field}` appears first."#);
        }
    }

    syn_fields.named = syn_fields
        .named
        .iter()
        .filter(|syn_field| {
            let Some(ident) = syn_field.ident.as_ref().map(|v| v.to_string()) else {
                return false;
            };

            parsed_fields.iter().any(|parsed_field| {
                if let Field::Normal { name } = &parsed_field {
                    *name == ident
                } else {
                    false
                }
            })
        })
        .cloned()
        .collect();

    let field_idents = syn_fields
        .named
        .iter()
        .filter_map(|v| v.ident.clone())
        .collect::<Vec<_>>();

    let struct_name = input.ident.clone();

    let binrw_impl = if cfg!(feature = "binrw") {
        quote::quote! {
            impl ::binrw::BinRead for #struct_name {
                type Args<'a> = ();

                fn read_options<R: std::io::Read + std::io::Seek>(
                    reader: &mut R,
                    endian: binrw::Endian,
                    args: Self::Args<'_>,
                ) -> ::binrw::BinResult<Self> {
                    use wezat::Wezat;

                    Self::from_bytes(reader).map_err(|e| binrw::Error::Custom {
                        pos: 0,
                        err: Box::new(e.to_string()),
                    })
                }
            }
        }
    } else {
        quote::quote! {}
    };

    let read_actions = {
        let mut actions = vec![];

        actions.push(quote::quote! {
            let mut pointers = ::std::collections::HashMap::<String, u32>::new();
            let _restore_pos = reader.stream_position()?;
        });

        for field in &parsed_fields {
            let field_name = field.name();
            let field_name_ident = quote::format_ident!("{}", field_name);

            match field {
                Field::Normal { name } => {
                    actions.push(quote::quote! {
                        let #field_name_ident = {
                            // pointer is available
                            if let Some(ptr) = pointers.get(#name).cloned() {
                                let _restore_pos = reader.stream_position()?;
                                reader.seek(std::io::SeekFrom::Start(ptr.into()))?;
                                let value = Wezat::from_bytes(reader)?;
                                reader.seek(std::io::SeekFrom::Start(_restore_pos))?;

                                value
                            }
                            else {
                                Wezat::from_bytes(reader)?
                            }
                        };
                    });
                }
                Field::Pointer { name: _, to_field } => {
                    actions.push(quote::quote! {
                        {
                            let ptr = wezat::Wezat::from_bytes(reader)?;
                            pointers.insert(#to_field.to_owned(), ptr);
                        }
                    });
                }
                Field::Length {
                    name,
                    len,
                    for_field,
                } => todo!(),
            }
        }

        actions
    };

    let write_actions = {
        let mut actions = vec![];

        actions.push(quote::quote! {
            type PointerType = u32;
            let mut pointers = ::std::collections::HashMap::<String, (u64, Option<String>)>::new();
            let _base_ptr = writer.stream_position()?;
        });

        // first pass
        for field in &parsed_fields {
            let ident = quote::format_ident!("{}", field.name());

            match field {
                Field::Normal { name } => {
                    actions.push(quote::quote! {
                        pointers.insert(#name.to_owned(), (writer.stream_position()?, None));
                        Wezat::write_bytes(&self.#ident, writer)?;
                    });
                }
                Field::Pointer { name, to_field } => {
                    // skip pointers on first pass, but note where they are
                    actions.push(quote::quote! {
                        pointers.insert(#name.to_owned(), (writer.stream_position()?, Some(#to_field.to_owned())));
                        writer.seek_relative(size_of::<PointerType>() as i64)?;
                    });
                }
                Field::Length {
                    name,
                    len,
                    for_field,
                } => todo!(),
            }
        }

        actions.push(quote::quote! {
            let _end_ptr = writer.stream_position()?;
        });

        actions.push(quote::quote! {
            for (field, (ptr, pointing_to)) in &pointers {
                // ignore non-pointer fields
                let Some(pointing_to) = pointing_to else {
                    continue;
                };

                // go to the address of the pointer and write it
                writer.seek(::std::io::SeekFrom::Start(*ptr))?;

                let (ptr, _) = pointers
                    .get(pointing_to)
                    .cloned()
                    .ok_or_else(|| format!("{field}: address of field {pointing_to} not found"))?;
                (ptr as PointerType).write_bytes(writer)?;
            }
        });

        actions.push(quote::quote! {
            writer.seek(::std::io::SeekFrom::Start(_end_ptr))?;
        });

        actions
    };

    let quoted = quote::quote! {
        #input
        impl wezat::Wezat for #struct_name {
            const MIN_SIZE: usize = 0;

            fn from_bytes(reader: &mut impl wezat::Reader) -> Result<Self, wezat::Error> {
                #(#read_actions)*

                Ok(Self {
                    #(
                        #field_idents,
                    )*
                })
            }

            fn write_bytes(&self, writer: &mut impl wezat::Writer) -> Result<(), wezat::Error> {
                #(#write_actions)*
                Ok(())
            }
        }

        #binrw_impl
    };

    quoted.into()
}

fn parse_fields(
    s: &mut syn::DataStruct,
) -> Result<(Vec<Field>, Vec<Dependency>), Box<dyn std::error::Error>> {
    let syn::Fields::Named(syn_fields) = &mut s.fields else {
        panic!("expected named fields")
    };

    let mut fields = vec![];
    let mut dependencies = vec![];

    let syn_idents = syn_fields
        .named
        .iter()
        .filter_map(|v| v.ident.clone())
        .collect::<Vec<_>>();

    for field in syn_fields.named.iter_mut() {
        let Some(ident_str) = field.ident.as_ref().map(|v| v.to_string()) else {
            continue;
        };

        match &mut field.ty {
            syn::Type::Array(array) => {
                // make sure len is a path
                let syn::Expr::Path(expr_path) = array.len.clone() else {
                    fields.push(Field::Normal { name: ident_str });
                    continue;
                };

                let segments = expr_path.path.segments;

                // more than one segment => not a field name
                if segments.len() != 1 {
                    fields.push(Field::Normal { name: ident_str });
                    continue;
                }

                let type_segment = &segments.first().unwrap().ident;

                let struct_is_using_variable_name = syn_idents
                    .iter()
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

                let elem = &array.elem;

                field.ty = syn::parse_quote! {
                    ::std::vec::Vec<#elem>
                };

                fields.push(Field::Normal { name: ident_str });
                continue;
            }
            syn::Type::Reference(type_reference) => {
                let syn::Type::Path(path) = type_reference.elem.as_ref() else {
                    fields.push(Field::Normal { name: ident_str });
                    continue;
                };

                let type_segment = {
                    if path.path.segments.len() != 1 {
                        fields.push(Field::Normal { name: ident_str });
                        continue;
                    }
                    path.path.segments.first().unwrap().ident.clone()
                };

                let struct_is_using_variable_name =
                    syn_idents.iter().any(|ident| ident == &type_segment);

                // if it's not a known field in the struct, ignore it
                if !struct_is_using_variable_name {
                    panic!("struct not using {type_segment}");
                }

                {
                    let field = type_segment.to_string();

                    fields.push(Field::Pointer {
                        name: ident_str.clone(),
                        to_field: field.clone(),
                    });

                    dependencies.push(Dependency {
                        field,
                        depends_on: ident_str,
                    });
                }

                let elem = &type_reference.elem;
                field.ty = syn::parse_quote! {
                    ::std::vec::Vec<#elem>
                };
            }
            _ => {
                fields.push(Field::Normal { name: ident_str });
            }
        }
    }

    Ok((fields, dependencies))
}
