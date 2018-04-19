use std::ffi::{CStr};
use std::slice;
use std::sync::Arc;
use std::str::FromStr;

use failure::ResultExt;
use libc;

use Result;
use ffi_utils::{CStringArray, CReprOf, RawPointerConverter};
use snips_nlu_ontology::{BuiltinEntity, BuiltinEntityKind, Language};
use snips_nlu_ontology_ffi_macros::{CBuiltinEntity, CBuiltinEntityArray};
use snips_nlu_ontology_parsers::BuiltinEntityParser;

#[repr(C)]
pub struct CBuiltinEntityParser(*const libc::c_void);

macro_rules! get_parser {
    ($opaque:ident) => {{
        let container: &$crate::CBuiltinEntityParser = unsafe { &*$opaque };
        let x = container.0 as *const ::snips_nlu_ontology_parsers::BuiltinEntityParser;
        unsafe { &*x }
    }};
}

pub fn create_builtin_entity_parser(
    ptr: *mut *const CBuiltinEntityParser,
    lang: *const libc::c_char,
) -> Result<()> {
    let lang = unsafe { CStr::from_ptr(lang) }.to_str()?;
    let lang = Language::from_str(&*lang.to_uppercase())?;
    let parser = BuiltinEntityParser::get(lang);

    let c_parser = CBuiltinEntityParser(Arc::into_raw(parser) as _).into_raw_pointer();

    unsafe {
        *ptr = c_parser;
    }
    Ok(())
}

pub fn extract_entity_c(
    ptr: *const CBuiltinEntityParser,
    sentence: *const libc::c_char,
    filter_entity_kinds: *const CStringArray,
    results: *mut *const CBuiltinEntityArray,
) -> Result<()> {
    let c_entities = extract_entity(ptr, sentence, filter_entity_kinds)?
        .into_iter()
        .map(CBuiltinEntity::from)
        .collect::<Vec<_>>();
    let c_entities = CBuiltinEntityArray::from(c_entities).into_raw_pointer();

    unsafe {
        *results = c_entities;
    }

    Ok(())
}

pub fn extract_entity_json(
    ptr: *const CBuiltinEntityParser,
    sentence: *const libc::c_char,
    filter_entity_kinds: *const CStringArray,
    results: *mut *const libc::c_char,
) -> Result<()> {
    let entities = extract_entity(ptr, sentence, filter_entity_kinds)?;
    let json = ::serde_json::to_string(&entities)?;

    let cs = convert_to_c_string!(json);
    unsafe { *results = cs }

    Ok(())
}

pub fn extract_entity(
    ptr: *const CBuiltinEntityParser,
    sentence: *const libc::c_char,
    filter_entity_kinds: *const CStringArray,
) -> Result<Vec<BuiltinEntity>> {
    let parser = get_parser!(ptr);
    let sentence = unsafe { CStr::from_ptr(sentence) }.to_str()?;

    let opt_filters: Option<Vec<_>> = if !filter_entity_kinds.is_null() {
        let filters = unsafe {
            let array = &*filter_entity_kinds;
            slice::from_raw_parts(array.data, array.size as usize)
        }.into_iter()
            .map(|&ptr| {
                Ok(unsafe { CStr::from_ptr(ptr) }
                    .to_str()
                    .map_err(::failure::Error::from)
                    .and_then(|s| {
                        BuiltinEntityKind::from_identifier(s)
                            .with_context(|_| format!("`{}` isn't a known builtin entity kind", s))
                            .map_err(::failure::Error::from)
                    })?)
            })
            .collect::<Result<Vec<_>>>()?;
        Some(filters)
    } else {
        None
    };
    let opt_filters = opt_filters.as_ref().map(|vec| vec.as_slice());

    Ok(parser.extract_entities(sentence, opt_filters))
}

pub fn destroy_builtin_entity_parser(ptr: *mut CBuiltinEntityParser) -> Result<()> {
    unsafe {
        let parser = CBuiltinEntityParser::from_raw_pointer(ptr)?.0;
        let _ = Arc::from_raw(parser);
    }
    Ok(())
}
