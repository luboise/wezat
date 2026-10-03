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
        ty: Box<syn::Type>,
    },
    Pointer {
        name: String,
        size: syn::Path,
        to_field: String,
    },
    Length {
        name: String,
        for_field: String,
    },
}

impl Field {
    pub fn name(&self) -> &str {
        match self {
            Field::Normal { name, ty: _ }
            | Field::Pointer { name, .. }
            | Field::Length { name, .. } => name.as_str(),
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
                if let Field::Normal { name, ty: _ } = &parsed_field {
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

            impl ::binrw::BinWrite for #struct_name {
                type Args<'a> = ();

                fn write_options<W: ::std::io::Write + std::io::Seek>(
                    &self,
                    writer: &mut W,
                    _: binrw::Endian,
                    _: Self::Args<'_>,
                ) -> binrw::prelude::BinResult<()> {
                    use wezat::Wezat;
                    Self::write_bytes(writer).map_err(|e| binrw::Error::Custom {
                        pos: 0,
                        err: Box::new(e.to_string()),
                    })
                }
            }
        }
    } else {
        quote::quote! {}
    };

    let type_is_vec = |ty: &syn::Type| -> bool {
        let syn::Type::Path(syn::TypePath { path, .. }) = ty else {
            return false;
        };
        path.segments.last().is_some_and(|v| v.ident == "Vec")
    };

    let read_actions = {
        let mut actions = vec![];

        // TODO: Merge pointers and lengths and use a struct from wezat instead?
        actions.push(quote::quote! {
            use wezat::Wezat;

            let mut pointers = ::std::collections::HashMap::<String, u32>::new();
            let mut lengths = ::std::collections::HashMap::<String, usize>::new();
            let _restore_pos = reader.stream_position()?;
        });

        for field in &parsed_fields {
            let field_name = field.name();
            let field_name_ident = quote::format_ident!("{}", field_name);

            match field {
                Field::Normal { name, ty } => {
                    let action = if type_is_vec(ty) {
                        quote::quote! {
                            let #field_name_ident = {
                                let Some(len) = lengths.get(#name).cloned() else {
                                    return Err(format!("internal wezat error: no length for field {}", #name).into());
                                };

                                (0..len)
                                    .map(|_| Wezat::from_bytes(reader))
                                    .collect::<Result<Vec<_>, _>>()?
                            };
                        }
                    } else {
                        quote::quote! {
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
                        }
                    };

                    actions.push(action);
                }
                Field::Pointer {
                    name: _,
                    size,
                    to_field,
                } => {
                    actions.push(quote::quote! {
                        {
                            let ptr: #size = wezat::Wezat::from_bytes(reader)?;
                            pointers.insert(#to_field.to_owned(), ptr);
                        }
                    });
                }
                Field::Length { name: _, for_field } => {
                    actions.push(quote::quote! {
                        {
                            let len: u32 = wezat::Wezat::from_bytes(reader)?;
                            lengths.insert(#for_field.to_owned(), len as usize);
                        }
                    });
                }
            }
        }

        actions
    };

    let write_actions = {
        let mut actions = vec![];

        actions.push(quote::quote! {
            use wezat::Wezat;

            type PointerType = u32;
            let mut pointers = ::std::collections::HashMap::<String, (u64, Option<String>)>::new();
            let _base_ptr = writer.stream_position()?;
        });

        // first pass
        for field in &parsed_fields {
            let ident = quote::format_ident!("{}", field.name());

            match field {
                Field::Normal { name, ty } => {
                    actions.push(quote::quote! {
                        pointers.insert(#name.to_owned(), (writer.stream_position()?, None));
                    });

                    if type_is_vec(ty) {
                        actions.push(quote::quote! {
                            for item in &self.#ident {
                                item.write_bytes(writer)?;
                            }
                        });
                    } else {
                        actions.push(quote::quote! {
                            self.#ident.write_bytes(writer)?;
                        });
                    }
                }
                Field::Pointer {
                    name,
                    size: _,
                    to_field,
                } => {
                    // skip pointers on first pass, but note where they are
                    actions.push(quote::quote! {
                        pointers.insert(#name.to_owned(), (writer.stream_position()?, Some(#to_field.to_owned())));
                        writer.seek_relative(size_of::<PointerType>() as i64)?;
                    });
                }
                Field::Length { name, for_field } => {
                    let for_ident = quote::format_ident!("{for_field}");

                    actions.push(quote::quote! {
                        {
                            pointers.insert(#name.to_owned(), (writer.stream_position()?, None));
                            let len = self.#for_ident.len() as PointerType;
                            len.write_bytes(writer)?;
                        }
                    });
                }
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
            type ReadArgs = ();

            fn from_bytes_ctx(_: &wezat::ReadContext<()>, reader: &mut impl wezat::Reader) -> Result<Self, wezat::Error> {
                #(#read_actions)*

                Ok(Self {
                    #(
                        #field_idents,
                    )*
                })
            }

            type WriteArgs = ();
            fn write_bytes_ctx(&self, _: &wezat::WriteContext<()>, writer: &mut impl wezat::Writer) -> Result<(), wezat::Error> {
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

    let mut retained_fields = vec![];
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

        let field_ty = field.ty.clone();

        match &mut field.ty {
            syn::Type::Array(array) => {
                // make sure len is a path
                let syn::Expr::Path(expr_path) = array.len.clone() else {
                    retained_fields.push(Field::Normal {
                        name: ident_str,
                        ty: field.ty.clone().into(),
                    });
                    continue;
                };

                let segments = expr_path.path.segments;

                // more than one segment => not a field name
                if segments.len() != 1 {
                    retained_fields.push(Field::Normal {
                        name: ident_str,
                        ty: field.ty.clone().into(),
                    });
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

                if let Some(len_field) = retained_fields
                    .iter_mut()
                    .find(|v| type_segment == v.name())
                {
                    *len_field = Field::Length {
                        name: len_field.name().to_owned(),
                        for_field: ident_str.clone(),
                    }
                } else {
                    panic!("no len for field {ident_str}");
                };

                retained_fields.push(Field::Normal {
                    name: ident_str,
                    ty: field.ty.clone().into(),
                });
                continue;
            }
            syn::Type::Ptr(ptr_reference) => {
                let Some((_eq, expr)) = &field.default else {
                    return Err(format!("no default on field {ident_str}").into());
                };

                let syn::Expr::Reference(r) = expr else {
                    panic!("expected reference for field {ident_str}");
                };

                let syn::Expr::Path(ptr_type_path) = r.expr.as_ref() else {
                    panic!("no path for field {ident_str}");
                };

                let type_segment = {
                    if ptr_type_path.path.segments.len() != 1 {
                        panic!("expected path of length 1");
                    }
                    ptr_type_path.path.segments.first().unwrap().ident.clone()
                };

                let struct_is_using_variable_name =
                    syn_idents.iter().any(|ident| ident == &type_segment);

                // if it's not a known field in the struct, ignore it
                if !struct_is_using_variable_name {
                    panic!("struct not using {type_segment}");
                }

                {
                    let field_str = type_segment.to_string();

                    let syn::Type::Ptr(type_ptr) = &field_ty else {
                        panic!("field {ident_str} is not a pointer");
                    };

                    let v = if let syn::Type::Path(v) = type_ptr.elem.as_ref() {
                        v
                    } else {
                        // default pointer type is u32
                        &syn::parse_quote!(u32)
                    };

                    retained_fields.push(Field::Pointer {
                        name: ident_str.clone(),
                        size: v.path.clone(),
                        to_field: field_str.clone(),
                    });

                    // dependencies.push(Dependency {
                    //     field,
                    //     depends_on: ident_str,
                    // });
                }

                let elem = &ptr_reference.elem;
                field.ty = syn::parse_quote! {
                    ::std::vec::Vec<#elem>
                };
            }
            _ => {
                retained_fields.push(Field::Normal {
                    name: ident_str,
                    ty: field.ty.clone().into(),
                });
            }
        }
    }

    Ok((retained_fields, dependencies))
}
