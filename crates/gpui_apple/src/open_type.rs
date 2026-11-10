use core_foundation::{
    array::{
        CFArray, CFArrayAppendValue, CFArrayCreateMutable, CFMutableArrayRef, kCFTypeArrayCallBacks,
    },
    base::{CFRelease, TCFType, kCFAllocatorDefault},
    dictionary::{
        CFDictionaryCreate, kCFTypeDictionaryKeyCallBacks, kCFTypeDictionaryValueCallBacks,
    },
    number::CFNumber,
    string::CFString,
};
use core_foundation_sys::locale::CFLocaleCopyPreferredLanguages;
use core_text::{
    font::{CTFont, cascade_list_for_languages},
    font_descriptor::{
        CTFontDescriptor, CTFontDescriptorCreateWithAttributes, kCTFontCascadeListAttribute,
        kCTFontFeatureSettingsAttribute, kCTFontSlantTrait, kCTFontSymbolicTrait,
        kCTFontWeightTrait, kCTFontWidthTrait,
    },
};
use font_kit::{
    family_handle::FamilyHandle, font::Font as FontKitFont, properties::Properties,
    source::SystemSource, sources::mem::MemSource,
};
use objc2_core_text::{kCTFontOpenTypeFeatureTag, kCTFontOpenTypeFeatureValue};
use open_gpui::{FontFallbacks, FontFeatures};

pub fn apply_features_and_fallbacks(
    font: &mut FontKitFont,
    features: &FontFeatures,
    fallbacks: Option<&FontFallbacks>,
    memory_source: &MemSource,
    system_source: &SystemSource,
) -> anyhow::Result<()> {
    unsafe {
        let mut keys = vec![kCTFontFeatureSettingsAttribute];
        let mut values = vec![generate_feature_array(features)];
        if let Some(fallbacks) = fallbacks
            && !fallbacks.fallback_list().is_empty()
        {
            keys.push(kCTFontCascadeListAttribute);
            values.push(generate_fallback_array(
                fallbacks,
                font,
                memory_source,
                system_source,
            ));
        }
        let attrs = CFDictionaryCreate(
            kCFAllocatorDefault,
            keys.as_ptr() as _,
            values.as_ptr() as _,
            keys.len() as isize,
            &kCFTypeDictionaryKeyCallBacks,
            &kCFTypeDictionaryValueCallBacks,
        );

        for value in &values {
            CFRelease(*value as _);
        }

        let new_descriptor = CTFontDescriptorCreateWithAttributes(attrs);
        CFRelease(attrs as _);
        let new_descriptor = CTFontDescriptor::wrap_under_create_rule(new_descriptor);
        // font-kit uses the older core-text wrappers. Borrow the same CF objects
        // for the generated API, then transfer its create-rule ownership back.
        let native_font = font.native_font();
        let generated_font = &*native_font
            .as_concrete_TypeRef()
            .cast::<objc2_core_text::CTFont>();
        let descriptor = &*new_descriptor
            .as_concrete_TypeRef()
            .cast::<objc2_core_text::CTFontDescriptor>();
        let new_font = generated_font.copy_with_attributes(0.0, std::ptr::null(), Some(descriptor));
        let new_font = CTFont::wrap_under_create_rule(
            objc2_core_foundation::CFRetained::into_raw(new_font)
                .as_ptr()
                .cast(),
        );
        *font = FontKitFont::from_native_font(&new_font);

        Ok(())
    }
}

fn generate_feature_array(features: &FontFeatures) -> CFMutableArrayRef {
    unsafe {
        let feature_array = CFArrayCreateMutable(kCFAllocatorDefault, 0, &kCFTypeArrayCallBacks);
        for (tag, value) in features.tag_value_list() {
            let keys = [
                kCTFontOpenTypeFeatureTag as *const _,
                kCTFontOpenTypeFeatureValue as *const _,
            ];
            let tag = CFString::new(tag);
            let value = CFNumber::from(*value as i32);
            let values = [tag.as_CFTypeRef(), value.as_CFTypeRef()];
            let dict = CFDictionaryCreate(
                kCFAllocatorDefault,
                &keys as *const _ as _,
                &values as *const _ as _,
                2,
                &kCFTypeDictionaryKeyCallBacks,
                &kCFTypeDictionaryValueCallBacks,
            );
            CFArrayAppendValue(feature_array, dict as _);
            CFRelease(dict as _);
        }
        feature_array
    }
}

fn generate_fallback_array(
    fallbacks: &FontFallbacks,
    font: &FontKitFont,
    memory_source: &MemSource,
    system_source: &SystemSource,
) -> CFMutableArrayRef {
    unsafe {
        let fallback_array = CFArrayCreateMutable(kCFAllocatorDefault, 0, &kCFTypeArrayCallBacks);
        let properties = font.properties();
        for family in fallbacks.fallback_list() {
            let family = open_gpui::font_name_with_fallbacks(family, ".AppleSystemUIFont");
            let fallback = memory_source
                .select_family_by_name(family)
                .ok()
                .and_then(|family| select_fallback_from_family(family, &properties))
                .or_else(|| {
                    system_source
                        .select_family_by_name(family)
                        .ok()
                        .and_then(|family| select_fallback_from_family(family, &properties))
                });
            if let Some(fallback) = fallback {
                let descriptor = fallback.native_font().copy_descriptor();
                CFArrayAppendValue(fallback_array, descriptor.as_concrete_TypeRef() as _);
            }
        }

        append_system_fallbacks(fallback_array, &font.native_font());
        fallback_array
    }
}

fn append_system_fallbacks(fallback_array: CFMutableArrayRef, font: &CTFont) {
    unsafe {
        let preferred_languages: CFArray<CFString> =
            CFArray::wrap_under_create_rule(CFLocaleCopyPreferredLanguages());

        let default_fallbacks = cascade_list_for_languages(font, &preferred_languages);

        for desc in default_fallbacks
            .iter()
            .filter(|desc| desc.font_path().is_some())
        {
            CFArrayAppendValue(fallback_array, desc.as_concrete_TypeRef() as _);
        }
    }
}

/// Rejects missing or invalid traits before font-kit reads their numeric properties.
pub(crate) fn font_has_valid_traits(font: &FontKitFont) -> bool {
    let traits = font.native_font().all_traits();
    let valid = unsafe {
        [
            kCTFontSymbolicTrait,
            kCTFontWidthTrait,
            kCTFontWeightTrait,
            kCTFontSlantTrait,
        ]
        .into_iter()
        .all(|key| {
            traits
                .find(key)
                .and_then(|value| value.downcast::<CFNumber>())
                .is_some()
        })
    };
    if !valid {
        log::error!(
            "Failed to read traits for font {:?}",
            font.postscript_name()
        );
    }
    valid
}

fn select_fallback_from_family(
    family: FamilyHandle,
    properties: &Properties,
) -> Option<FontKitFont> {
    let mut fonts = Vec::new();
    let mut candidates = Vec::new();
    for handle in family.fonts() {
        let font = match handle.load() {
            Ok(font) => font,
            Err(error) => {
                log::warn!("Error loading fallback font: {error:?}");
                continue;
            }
        };
        if !font_has_valid_traits(&font) {
            continue;
        }
        candidates.push(font.properties());
        fonts.push(font);
    }
    let index = font_kit::matching::find_best_match(&candidates, properties).ok()?;
    Some(fonts.swap_remove(index))
}
