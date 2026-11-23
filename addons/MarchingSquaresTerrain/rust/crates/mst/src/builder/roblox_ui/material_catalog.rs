//! What the material picker offers and where an applied material lands, kept as plain
//! values so the rules can be tested without the editor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shelf {
    Recent = 0,
    Project = 1,
    BuiltIn = 2,
}
impl Shelf {
    pub const ALL: [Shelf; 3] = [Shelf::Recent, Shelf::Project, Shelf::BuiltIn];
    pub fn from_i64(v: i64) -> Shelf {
        Self::ALL
            .into_iter()
            .find(|s| *s as i64 == v)
            .unwrap_or(Shelf::Project)
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Origin {
    Slot(usize),
    File(String),
    Prefab(usize),
    TerrainSlot { preset: String, slot: usize },
}
impl Origin {
    pub fn key(&self) -> String {
        match self {
            Origin::Slot(i) => format!("slot:{i}"),
            Origin::File(p) => p.clone(),
            Origin::Prefab(i) => format!("prefab:{i}"),
            Origin::TerrainSlot { preset, slot } => format!("terrain:{preset}#{slot}"),
        }
    }
    pub fn shelf(&self) -> Shelf {
        match self {
            Origin::Slot(_) | Origin::File(_) => Shelf::Project,
            Origin::Prefab(_) | Origin::TerrainSlot { .. } => Shelf::BuiltIn,
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub name: String,
    pub origin: Origin,
}
pub fn display_name(path: &str) -> String {
    let file = path.rsplit('/').next().unwrap_or(path);
    let stem = file.split('.').next().unwrap_or(file);
    stem.split(['_', '-', ' '])
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(f) => f.to_uppercase().chain(c).collect(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
pub fn matches(name: &str, query: &str) -> bool {
    let name = name.to_lowercase();
    query
        .split_whitespace()
        .all(|w| name.contains(&w.to_lowercase()))
}
pub fn visible(entries: &[Entry], recent: &[String], shelf: Shelf, query: &str) -> Vec<usize> {
    let on_shelf: Vec<usize> = match shelf {
        Shelf::Recent => recent
            .iter()
            .filter_map(|k| entries.iter().position(|e| e.origin.key() == *k))
            .collect(),
        _ => (0..entries.len())
            .filter(|i| entries[*i].origin.shelf() == shelf)
            .collect(),
    };
    on_shelf
        .into_iter()
        .filter(|i| matches(&entries[*i].name, query))
        .collect()
}
pub fn opening_shelf(entries: &[Entry], chosen: Option<Shelf>) -> Shelf {
    chosen.unwrap_or_else(|| {
        if entries.iter().any(|e| e.origin.shelf() == Shelf::Project) {
            Shelf::Project
        } else {
            Shelf::BuiltIn
        }
    })
}
pub fn remember(recent: &mut Vec<String>, key: &str, cap: usize) {
    recent.retain(|k| k != key);
    recent.insert(0, key.to_string());
    recent.truncate(cap);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlotChoice {
    Reuse(usize),
    Append { at: usize },
}
pub fn slot_for(slot_keys: &[Option<String>], slots_in_use: usize, key: &str) -> SlotChoice {
    if let Some(i) = slot_keys.iter().position(|k| k.as_deref() == Some(key)) {
        return SlotChoice::Reuse(i);
    }
    SlotChoice::Append {
        at: slot_keys.len().max(slots_in_use),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn entry(name: &str, origin: Origin) -> Entry {
        Entry {
            name: name.into(),
            origin,
        }
    }
    fn sample() -> Vec<Entry> {
        vec![
            entry("Slot 0: Grass", Origin::Slot(0)),
            entry("Wood Planks", Origin::File("res://m/wood_planks.tres".into())),
            entry("Grid", Origin::Prefab(0)),
            entry(
                "Dark Rock",
                Origin::TerrainSlot {
                    preset: "res://p.tres".into(),
                    slot: 6,
                },
            ),
        ]
    }
    #[test]
    fn display_name_title_cases_the_file_stem() {
        assert_eq!(display_name("res://a/b/wood_planks.tres"), "Wood Planks");
        assert_eq!(display_name("roof-tiles.material"), "Roof Tiles");
        assert_eq!(display_name(""), "");
    }
    #[test]
    fn search_needs_every_word_in_any_case() {
        assert!(matches("Wood Planks", "plank"));
        assert!(matches("Wood Planks", "WOOD pl"));
        assert!(!matches("Wood Planks", "wood stone"));
        assert!(matches("Anything", "  "));
    }
    #[test]
    fn shelves_split_project_and_built_in() {
        let e = sample();
        assert_eq!(visible(&e, &[], Shelf::Project, ""), vec![0, 1]);
        assert_eq!(visible(&e, &[], Shelf::BuiltIn, ""), vec![2, 3]);
        assert_eq!(visible(&e, &[], Shelf::BuiltIn, "rock"), vec![3]);
    }
    #[test]
    fn recent_follows_the_order_of_use_and_skips_vanished_entries() {
        let e = sample();
        let mut recent = Vec::new();
        remember(&mut recent, &e[2].origin.key(), 3);
        remember(&mut recent, "res://gone.tres", 3);
        remember(&mut recent, &e[1].origin.key(), 3);
        remember(&mut recent, &e[2].origin.key(), 3);
        assert_eq!(visible(&e, &recent, Shelf::Recent, ""), vec![2, 1]);
        remember(&mut recent, "a", 3);
        assert_eq!(recent.len(), 3);
    }
    #[test]
    fn a_known_material_reuses_its_slot() {
        let keys = vec![None, Some("prefab:0".to_string())];
        assert_eq!(slot_for(&keys, 2, "prefab:0"), SlotChoice::Reuse(1));
    }
    #[test]
    fn a_new_material_goes_after_every_slot_the_faces_use() {
        let keys = vec![Some("res://a.tres".to_string())];
        assert_eq!(slot_for(&keys, 1, "prefab:1"), SlotChoice::Append { at: 1 });
        assert_eq!(slot_for(&keys, 4, "prefab:1"), SlotChoice::Append { at: 4 });
        assert_eq!(slot_for(&[], 0, "x"), SlotChoice::Append { at: 0 });
    }
    #[test]
    fn an_empty_project_opens_on_built_in_unless_a_shelf_was_chosen() {
        let e = sample();
        assert_eq!(opening_shelf(&e, None), Shelf::Project);
        assert_eq!(opening_shelf(&e[2..], None), Shelf::BuiltIn);
        assert_eq!(opening_shelf(&e[2..], Some(Shelf::Recent)), Shelf::Recent);
    }
    #[test]
    fn shelf_from_unknown_number_is_project() {
        assert_eq!(Shelf::from_i64(2), Shelf::BuiltIn);
        assert_eq!(Shelf::from_i64(9), Shelf::Project);
    }
}
