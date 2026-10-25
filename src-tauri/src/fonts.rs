//! Installed font families for the font picker (PLAN.md 9.5), read through DirectWrite. Also
//! answers whether a Japanese UI font is present: Yu Gothic UI or Meiryo UI. Without one, the
//! settings point to the Windows feature "Japanese Supplemental Fonts".

use std::collections::BTreeSet;

use serde::Serialize;
use windows::core::{w, BOOL};
use windows::Win32::Graphics::DirectWrite::{DWriteCreateFactory, IDWriteFactory, IDWriteFontCollection, IDWriteLocalizedStrings, DWRITE_FACTORY_TYPE_SHARED};

#[derive(Serialize, Clone, Debug)]
pub struct FontList {
    /// Family names, sorted, without vertical (@) and symbol-only variants.
    pub families: Vec<String>,
    /// Yu Gothic UI or Meiryo UI is installed (the Japanese fallbacks of the overlay).
    pub japanese_ui_font: bool,
}

pub fn list() -> FontList {
    let families = match system_families() {
        Ok(f) => f,
        Err(e) => {
            tracing::warn!(error = %e, "font list unavailable");
            BTreeSet::new()
        }
    };
    let japanese_ui_font = families.iter().any(|f| f == "Yu Gothic UI" || f == "Meiryo UI");
    FontList { families: families.into_iter().filter(|f| !f.starts_with('@')).collect(), japanese_ui_font }
}

fn system_families() -> windows::core::Result<BTreeSet<String>> {
    let mut out = BTreeSet::new();
    unsafe {
        let factory: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;
        let mut collection: Option<IDWriteFontCollection> = None;
        factory.GetSystemFontCollection(&mut collection, false)?;
        let Some(collection) = collection else { return Ok(out) };
        for i in 0..collection.GetFontFamilyCount() {
            let names = collection.GetFontFamily(i)?.GetFamilyNames()?;
            if let Some(name) = english_name(&names) {
                out.insert(name);
            }
        }
    }
    Ok(out)
}

/// The en-us name, or the first one the font has.
unsafe fn english_name(names: &IDWriteLocalizedStrings) -> Option<String> {
    let mut index = 0u32;
    let mut exists = BOOL(0);
    unsafe {
        names.FindLocaleName(w!("en-us"), &mut index, &mut exists).ok()?;
        if !exists.as_bool() {
            index = 0;
        }
        let len = names.GetStringLength(index).ok()? as usize;
        let mut buf = vec![0u16; len + 1];
        names.GetString(index, &mut buf).ok()?;
        Some(String::from_utf16_lossy(&buf[..len]))
    }
}
