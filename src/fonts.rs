//! Licensed, app-local font assets; no system installation. See assets/fonts/manifest.json.
pub struct BundledFont {
    pub name: &'static str,
    pub family: &'static str,
    pub data: &'static [u8],
    pub license: &'static str,
}
pub const BUNDLED: &[BundledFont] = &[
    BundledFont {
        name: "IBM Plex Mono",
        family: "IBM Plex Mono",
        data: include_bytes!("../assets/fonts/ibmplexmono/font.ttf"),
        license: include_str!("../assets/fonts/ibmplexmono/OFL.txt"),
    },
    BundledFont {
        name: "JetBrains Mono",
        family: "JetBrains Mono",
        data: include_bytes!("../assets/fonts/jetbrainsmono/font.ttf"),
        license: include_str!("../assets/fonts/jetbrainsmono/OFL.txt"),
    },
    BundledFont {
        name: "Fira Code",
        family: "Fira Code",
        data: include_bytes!("../assets/fonts/firacode/font.ttf"),
        license: include_str!("../assets/fonts/firacode/OFL.txt"),
    },
    BundledFont {
        name: "Cascadia Code",
        family: "Cascadia Code",
        data: include_bytes!("../assets/fonts/cascadiacode/font.ttf"),
        license: include_str!("../assets/fonts/cascadiacode/OFL.txt"),
    },
    BundledFont {
        name: "Cascadia Mono",
        family: "Cascadia Mono",
        data: include_bytes!("../assets/fonts/cascadiamono/font.ttf"),
        license: include_str!("../assets/fonts/cascadiamono/OFL.txt"),
    },
    BundledFont {
        name: "Source Code Pro",
        family: "Source Code Pro",
        data: include_bytes!("../assets/fonts/sourcecodepro/font.ttf"),
        license: include_str!("../assets/fonts/sourcecodepro/OFL.txt"),
    },
    BundledFont {
        name: "Inconsolata",
        family: "Inconsolata",
        data: include_bytes!("../assets/fonts/inconsolata/font.ttf"),
        license: include_str!("../assets/fonts/inconsolata/OFL.txt"),
    },
    BundledFont {
        name: "Roboto Mono",
        family: "Roboto Mono",
        data: include_bytes!("../assets/fonts/robotomono/font.ttf"),
        license: include_str!("../assets/fonts/robotomono/OFL.txt"),
    },
    BundledFont {
        name: "Space Mono",
        family: "Space Mono",
        data: include_bytes!("../assets/fonts/spacemono/font.ttf"),
        license: include_str!("../assets/fonts/spacemono/OFL.txt"),
    },
    BundledFont {
        name: "PT Mono",
        family: "PT Mono",
        data: include_bytes!("../assets/fonts/ptmono/font.ttf"),
        license: include_str!("../assets/fonts/ptmono/OFL.txt"),
    },
    BundledFont {
        name: "DM Mono",
        family: "DM Mono",
        data: include_bytes!("../assets/fonts/dmmono/font.ttf"),
        license: include_str!("../assets/fonts/dmmono/OFL.txt"),
    },
    BundledFont {
        name: "Victor Mono",
        family: "Victor Mono",
        data: include_bytes!("../assets/fonts/victormono/font.ttf"),
        license: include_str!("../assets/fonts/victormono/OFL.txt"),
    },
    BundledFont {
        name: "Ubuntu Sans Mono",
        family: "Ubuntu Sans Mono",
        data: include_bytes!("../assets/fonts/ubuntusansmono/font.ttf"),
        license: include_str!("../assets/fonts/ubuntusansmono/LICENSE.txt"),
    },
    BundledFont {
        name: "Monaspace Neon",
        family: "Monaspace Neon",
        data: include_bytes!("../assets/fonts/monaspaceneon/font.ttf"),
        license: include_str!("../assets/fonts/monaspaceneon/LICENSE.txt"),
    },
    BundledFont {
        name: "Iosevka",
        family: "Iosevka",
        data: include_bytes!("../assets/fonts/iosevka/font.ttf"),
        license: include_str!("../assets/fonts/iosevka/LICENSE.txt"),
    },
];

pub fn load_bundled(db: &mut fontdb::Database) {
    for font in BUNDLED {
        db.load_font_data(font.data.to_vec());
    }
}
pub fn choices() -> &'static [String] {
    static CHOICES: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    CHOICES.get_or_init(|| {
        let mut names: Vec<String> = BUNDLED.iter().map(|f| f.name.to_owned()).collect();
        let mut db = fontdb::Database::new();
        db.load_system_fonts();
        let mut installed: Vec<String> = db
            .faces()
            .filter(|f| f.monospaced)
            .flat_map(|f| f.families.iter().map(|(name, _)| name.clone()))
            .filter(|name| !names.contains(name))
            .collect();
        installed.sort();
        installed.dedup();
        names.extend(installed);
        names
    })
}
pub fn notices() -> String {
    let mut text = String::from("Juna bundled font notices\n\n");
    for font in BUNDLED {
        text.push_str(font.name);
        text.push_str("\n\n");
        text.push_str(font.license);
        text.push_str("\n\n");
    }
    text
}
pub fn load_primary(db: &mut fontdb::Database, preferred: &str) -> Option<fontdb::ID> {
    if let Some(font) = BUNDLED
        .iter()
        .find(|font| font.name == preferred || font.family == preferred)
    {
        db.load_font_data(font.data.to_vec());
        return db.query(&fontdb::Query {
            families: &[fontdb::Family::Name(font.family)],
            ..Default::default()
        });
    }
    None
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_fifteen_faces_resolve_without_system_fonts() {
        assert_eq!(BUNDLED.len(), 15);
        let mut db = fontdb::Database::new();
        load_bundled(&mut db);
        for font in BUNDLED {
            let id = db
                .query(&fontdb::Query {
                    families: &[fontdb::Family::Name(font.family)],
                    ..Default::default()
                })
                .expect(font.name);
            assert!(db
                .face(id)
                .unwrap()
                .families
                .iter()
                .any(|(name, _)| name == font.family));
            db.with_face_data(id, |data, index| {
                let face = swash::FontRef::from_index(data, index as usize).expect(font.name);
                // Box drawing and CJK are allowed to use the separately tested fallback faces.
                for ch in "0O1Il{}[]".chars() {
                    assert_ne!(face.charmap().map(ch), 0, "{} missing {ch}", font.name);
                }
            })
            .unwrap();
        }
    }
}
