use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use crate::fs_scan::{display_path, tracked_text_files};

pub(crate) fn scan_theme_drift(root: &Path) -> Result<(), ()> {
    println!("==> scan theme drift");

    let mut failures = Vec::new();
    scan_theme_recipe_coverage(root, &mut failures);
    scan_theme_palette_coverage(root, &mut failures);

    if failures.is_empty() {
        println!("theme drift scan passed");
        Ok(())
    } else {
        eprintln!("theme drift scan failed:");
        for failure in failures {
            eprintln!("  {failure}");
        }
        Err(())
    }
}

fn scan_theme_recipe_coverage(root: &Path, failures: &mut Vec<String>) {
    let theme_dir = root.join("crates/ui_components/src/theme");
    let recipes_path = theme_dir.join("recipes.rs");
    let Ok(recipes) = fs::read_to_string(&recipes_path) else {
        failures.push("crates/ui_components/src/theme/recipes.rs: failed to read file".to_string());
        return;
    };

    let recipe_definitions = recipe_definitions(&recipes);
    let recipe_calls = theme_recipe_calls(root);
    let recipe_catalog = theme_recipe_catalog(&recipes);

    for recipe in &recipe_calls {
        if !recipe_definitions.contains(recipe) {
            failures.push(format!(
                "theme recipe `{recipe}` is called by a component but has no `ThemeResolver::{recipe}` definition in theme/recipes.rs"
            ));
        }
        if !recipe_catalog.contains(recipe) {
            failures.push(format!(
                "theme recipe `{recipe}` is missing from `THEME_RECIPE_CATALOG` in theme/recipes.rs"
            ));
        }
    }

    for recipe in recipe_definitions {
        if !recipe_catalog.contains(&recipe) {
            failures.push(format!(
                "theme recipe `{recipe}` is defined but missing from `THEME_RECIPE_CATALOG`"
            ));
        }
    }

    for path in tracked_text_files(root, is_ui_component_rust_file) {
        if path.starts_with(&theme_dir) {
            continue;
        }

        let relative = display_path(root, &path);
        let Ok(contents) = fs::read_to_string(&path) else {
            failures.push(format!("{relative}: failed to read file"));
            continue;
        };

        if contents.contains("impl ThemeResolver") {
            failures.push(format!(
                "{relative}: component files must not extend ThemeResolver; move color recipes to crates/ui_components/src/theme/recipes.rs"
            ));
        }
    }
}

fn scan_theme_palette_coverage(root: &Path, failures: &mut Vec<String>) {
    let path = root.join("crates/ui_components/src/theme/palette.rs");
    let relative = "crates/ui_components/src/theme/palette.rs";
    let Ok(contents) = fs::read_to_string(&path) else {
        failures.push(format!("{relative}: failed to read file"));
        return;
    };

    let themes = theme_palette_entries(&contents);
    for required in [
        "LIGHT_THEME_COLORS",
        "DARK_THEME_COLORS",
        "HIGH_CONTRAST_THEME_COLORS",
    ] {
        if !themes.contains_key(required) {
            failures.push(format!("{relative}: missing `{required}` color table"));
        }
    }

    let Some(light) = themes.get("LIGHT_THEME_COLORS") else {
        return;
    };

    for (name, entries) in &themes {
        if entries != light {
            let missing = light.difference(entries).cloned().collect::<Vec<_>>();
            let extra = entries.difference(light).cloned().collect::<Vec<_>>();
            if !missing.is_empty() {
                failures.push(format!(
                    "{relative}: `{name}` is missing token/state entries: {}",
                    missing.join(", ")
                ));
            }
            if !extra.is_empty() {
                failures.push(format!(
                    "{relative}: `{name}` has extra token/state entries: {}",
                    extra.join(", ")
                ));
            }
        }
    }
}

fn recipe_definitions(contents: &str) -> BTreeSet<String> {
    contents
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if !line.starts_with("pub(crate)") {
                return None;
            }
            let start = line.find("fn ")? + 3;
            let name = line[start..].split_once('(')?.0.trim();
            name.ends_with("_colors").then(|| name.to_string())
        })
        .collect()
}

fn theme_recipe_calls(root: &Path) -> BTreeSet<String> {
    let mut calls = BTreeSet::new();
    for path in tracked_text_files(root, is_ui_component_rust_file) {
        if path
            .strip_prefix(root)
            .ok()
            .is_some_and(|relative| relative.starts_with("crates/ui_components/src/theme"))
        {
            continue;
        }

        let Ok(contents) = fs::read_to_string(&path) else {
            continue;
        };
        for name in theme_resolver_method_names(&contents) {
            if name.ends_with("_colors") {
                calls.insert(name);
            }
        }
    }
    calls
}

fn theme_recipe_catalog(contents: &str) -> BTreeSet<String> {
    let Some(start) = contents.find("THEME_RECIPE_CATALOG") else {
        return BTreeSet::new();
    };
    let Some(open) = contents[start..].find('[').map(|index| start + index) else {
        return BTreeSet::new();
    };
    let Some(close) = contents[open..].find("];").map(|index| open + index) else {
        return BTreeSet::new();
    };

    quoted_strings(&contents[open..close]).into_iter().collect()
}

fn theme_resolver_method_names(contents: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut rest = contents;
    while let Some(index) = rest.find("ThemeResolver::") {
        rest = &rest[index + "ThemeResolver::".len()..];
        let name = rest
            .chars()
            .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
            .collect::<String>();
        if !name.is_empty() {
            names.push(name);
        }
    }
    names
}

fn theme_palette_entries(contents: &str) -> BTreeMap<String, BTreeSet<String>> {
    let mut themes = BTreeMap::new();
    for name in [
        "LIGHT_THEME_COLORS",
        "DARK_THEME_COLORS",
        "HIGH_CONTRAST_THEME_COLORS",
    ] {
        if let Some(body) = const_slice_body(contents, name) {
            themes.insert(name.to_string(), theme_color_entries(body));
        }
    }
    themes
}

fn const_slice_body<'a>(contents: &'a str, name: &str) -> Option<&'a str> {
    let start = contents.find(name)?;
    let open = contents[start..]
        .find("&[")
        .map(|index| start + index + 2)?;
    let close = contents[open..].find("];").map(|index| open + index)?;
    Some(&contents[open..close])
}

fn theme_color_entries(body: &str) -> BTreeSet<String> {
    let mut entries = BTreeSet::new();
    let mut rest = body;
    while let Some(index) = rest.find("ThemeColor::new(") {
        rest = &rest[index + "ThemeColor::new(".len()..];
        let Some(close) = rest.find(')') else {
            break;
        };
        let args = &rest[..close];
        let mut parts = args.split(',').map(str::trim);
        let token = parts.next().unwrap_or_default();
        let state = parts.next().unwrap_or_default();
        if !token.is_empty() && !state.is_empty() {
            entries.insert(format!("{token}/{state}"));
        }
        rest = &rest[close + 1..];
    }
    entries
}

fn quoted_strings(contents: &str) -> Vec<String> {
    let mut values = Vec::new();
    let mut rest = contents;
    while let Some(start) = rest.find('"') {
        rest = &rest[start + 1..];
        let Some(end) = rest.find('"') else {
            break;
        };
        values.push(rest[..end].to_string());
        rest = &rest[end + 1..];
    }
    values
}

fn is_ui_component_rust_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension == "rs")
        && path.components().any(|component| {
            component.as_os_str() == "ui_components"
                || component.as_os_str() == "crates\\ui_components"
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_recipe_scan_tracks_public_component_recipes_only() {
        let contents = r#"
const THEME_RECIPE_CATALOG: &[&str] = &[
    "button_colors",
];

impl ThemeResolver {
    pub(crate) const fn button_colors(tokens: ThemeTokens) -> ButtonColors {
        Self::accent_button_colors(tokens)
    }

    const fn accent_button_colors(tokens: ThemeTokens) -> ButtonColors {
        todo!()
    }
}
"#;

        assert_eq!(
            recipe_definitions(contents),
            BTreeSet::from(["button_colors".to_string()])
        );
        assert_eq!(
            theme_recipe_catalog(contents),
            BTreeSet::from(["button_colors".to_string()])
        );
    }

    #[test]
    fn theme_palette_scan_compares_token_state_shape() {
        let contents = r#"
const LIGHT_THEME_COLORS: &[ThemeColor] = &[
    ThemeColor::new(semantic::SURFACE, ColorState::Default, 0xffffff),
    ThemeColor::new(semantic::SURFACE_MUTED, ColorState::Hover, 0xf1f5ee),
];

const DARK_THEME_COLORS: &[ThemeColor] = &[
    ThemeColor::new(semantic::SURFACE, ColorState::Default, 0x000000),
];

const HIGH_CONTRAST_THEME_COLORS: &[ThemeColor] = &[
    ThemeColor::new(semantic::SURFACE, ColorState::Default, 0xffffff),
    ThemeColor::new(semantic::SURFACE_MUTED, ColorState::Hover, 0xffffff),
];
"#;
        let themes = theme_palette_entries(contents);

        assert!(themes["LIGHT_THEME_COLORS"].contains("semantic::SURFACE/ColorState::Default"));
        assert!(
            themes["LIGHT_THEME_COLORS"]
                .difference(&themes["DARK_THEME_COLORS"])
                .any(|entry| entry == "semantic::SURFACE_MUTED/ColorState::Hover")
        );
    }
}
