use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const ALL: &str = "*";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Item {
    pub name: String,
    pub ext: String,
}
impl Item {
    pub fn animated(&self) -> bool {
        matches!(self.ext.as_str(), "mp4" | "webm" | "gif")
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Choice {
    pub id: String,
    pub fit: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Profile {
    pub items: BTreeMap<String, Item>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub(super) pending_deletions: BTreeMap<String, Item>,
    pub screens: BTreeMap<String, Choice>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_wallpaper: Option<crate::desktop::background::SystemWallpaper>,
    pub autostart: Option<bool>,
    pub power: bool,
    pub paused: bool,
    #[serde(default)]
    pub pause_in_overview: bool,
    #[serde(default)]
    pub theme: crate::preferences::Appearance,
    #[serde(default)]
    pub language: crate::preferences::Language,
}
impl Profile {
    pub fn sorted_ids(&self) -> Vec<String> {
        let mut items: Vec<_> = self.items.iter().collect();
        items.sort_by(|(_, a), (_, b)| a.name.cmp(&b.name));
        items.into_iter().map(|(id, _)| id.clone()).collect()
    }

    pub fn assign(&mut self, screen: &str, choice: Choice) -> Result<()> {
        let item = self.items.get(&choice.id).context("Unknown library item")?;
        if item.animated() && self.autostart.is_none() {
            self.autostart = Some(true);
        }
        if screen == ALL {
            self.screens.clear();
        }
        self.screens.insert(screen.to_owned(), choice);
        Ok(())
    }

    pub fn has_animation(&self) -> bool {
        self.screens.values().any(|c| self.items[&c.id].animated())
    }

    pub fn clear_wallpapers(&mut self) {
        self.screens.clear();
        self.system_wallpaper = None;
    }

    pub fn restores_desktop_on_delete(&self, id: &str) -> bool {
        self.system_wallpaper.is_some()
            && (self.screens.get(ALL).is_some_and(|c| c.id == id)
                || (!self.screens.is_empty() && self.screens.values().all(|c| c.id == id)))
    }

    pub fn delete_item(&mut self, id: &str) -> Result<()> {
        let item = self.items.remove(id).context("Unknown library item")?;
        if self.restores_desktop_on_delete(id) {
            self.system_wallpaper = None;
        }
        self.screens.retain(|_, c| c.id != id);
        self.pending_deletions.insert(id.to_owned(), item);
        Ok(())
    }
}
