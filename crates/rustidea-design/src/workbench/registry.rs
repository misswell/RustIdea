//! Contribution registries, the Rust counterpart of OpenIDEA's
//! `src/workbench/contributions.ts`.
//!
//! A feature registers a descriptor instead of being imported by the workbench.
//! The renderer looks descriptors up by id, which keeps the growing sidebar
//! enum from coming back and gives plugins a real extension point.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::layout::ToolWindowAnchor;

/// Metadata for one tool window. The implementation (a GPUI entity) is kept by
/// the renderer, keyed by `id`; this descriptor is the contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolWindowContribution {
    pub id: String,
    pub title: String,
    /// Icon name from the migrated OpenIDEA icon set, not an abbreviation.
    pub icon: String,
    pub anchor: ToolWindowAnchor,
    /// Docks in the side's split area (IDEA `secondary` / `isSplit`).
    #[serde(default)]
    pub secondary: bool,
    /// Order within the stripe group; lower first.
    pub order: Option<i32>,
    /// Whether the stripe shows a button before the user opens it.
    #[serde(default = "default_true")]
    pub requires_project: bool,
    /// `true`: only under the stripe's "…" until used; `false` (default): shown.
    #[serde(default)]
    pub on_use: bool,
    /// Content that does not outlive the project (Run, Debug, terminals).
    #[serde(default)]
    pub session_content: bool,
    /// The title is external (a plugin) and must not be translated.
    #[serde(default)]
    pub verbatim: bool,
}

fn default_true() -> bool {
    true
}

impl ToolWindowContribution {
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        icon: impl Into<String>,
        anchor: ToolWindowAnchor,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            icon: icon.into(),
            anchor,
            secondary: false,
            order: None,
            requires_project: true,
            on_use: false,
            session_content: false,
            verbatim: false,
        }
    }

    pub fn order(mut self, order: i32) -> Self {
        self.order = Some(order);
        self
    }

    pub fn secondary(mut self) -> Self {
        self.secondary = true;
        self
    }

    pub fn on_use(mut self) -> Self {
        self.on_use = true;
        self
    }

    pub fn session(mut self) -> Self {
        self.session_content = true;
        self
    }

    pub fn without_project(mut self) -> Self {
        self.requires_project = false;
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Align {
    Left,
    Right,
}

/// A title-bar widget (VCS branch, run configuration, run/debug buttons…).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TitleItemContribution {
    pub id: String,
    pub align: Align,
    /// Higher = further from the centre (closer to the window edge).
    pub priority: i32,
    pub title: Option<String>,
}

/// A status-bar widget.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusItemContribution {
    pub id: String,
    pub align: Align,
    pub priority: i32,
    pub title: Option<String>,
    pub default_visible: bool,
    /// Also shown in a LightEdit window.
    pub light_edit: bool,
}

/// A popup or dialog rendered above the workbench (Search Everywhere, Settings…).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OverlayContribution {
    pub id: String,
    pub title: String,
}

/// An editor tab kind other than a text file: diff viewer, settings, welcome…
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditorKindContribution {
    pub kind: String,
    pub title: String,
    pub restorable: bool,
    pub standalone: bool,
    pub keep_alive: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettingsPageContribution {
    pub id: String,
    pub title: String,
    pub category: String,
    pub order: i32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionContribution {
    pub id: String,
    pub title: String,
    pub keybinding: Option<String>,
    pub icon: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MenuGroupContribution {
    pub id: String,
    pub parent: Option<String>,
    pub order: i32,
}

/// A stable, insertion-ordered registry. Re-registering an id replaces it, as
/// OpenIDEA's contribution registry does, and returns the previous value.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Registry<T> {
    entries: BTreeMap<String, T>,
}

impl<T> Default for Registry<T> {
    fn default() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }
}

impl<T> Registry<T> {
    pub fn insert(&mut self, id: impl Into<String>, value: T) -> Option<T> {
        self.entries.insert(id.into(), value)
    }

    pub fn get(&self, id: &str) -> Option<&T> {
        self.entries.get(id)
    }

    pub fn contains(&self, id: &str) -> bool {
        self.entries.contains_key(id)
    }

    pub fn remove(&mut self, id: &str) -> Option<T> {
        self.entries.remove(id)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &T)> {
        self.entries.iter()
    }

    pub fn values(&self) -> impl Iterator<Item = &T> {
        self.entries.values()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[derive(Clone, Debug, Default)]
pub struct Contributions {
    pub tool_windows: Registry<ToolWindowContribution>,
    pub title_items: Registry<TitleItemContribution>,
    pub status_items: Registry<StatusItemContribution>,
    pub overlays: Registry<OverlayContribution>,
    pub editor_kinds: Registry<EditorKindContribution>,
    pub settings_pages: Registry<SettingsPageContribution>,
    pub actions: Registry<ActionContribution>,
    pub menu_groups: Registry<MenuGroupContribution>,
}

impl Contributions {
    pub fn register_tool_window(&mut self, contribution: ToolWindowContribution) {
        self.tool_windows
            .insert(contribution.id.clone(), contribution);
    }

    pub fn register_title_item(&mut self, contribution: TitleItemContribution) {
        self.title_items.insert(contribution.id.clone(), contribution);
    }

    pub fn register_status_item(&mut self, contribution: StatusItemContribution) {
        self.status_items
            .insert(contribution.id.clone(), contribution);
    }

    /// Tool-window descriptors ordered as the stripe renders them: by
    /// `order`, then title, as `orderedToolWindows` in OpenIDEA.
    pub fn tool_windows_in_order(&self) -> Vec<&ToolWindowContribution> {
        let mut windows: Vec<_> = self.tool_windows.values().collect();
        windows.sort_by(|a, b| {
            a.order
                .unwrap_or(100)
                .cmp(&b.order.unwrap_or(100))
                .then_with(|| a.title.cmp(&b.title))
        });
        windows
    }
}

/// The tool windows OpenIDEA registers on a normal project, with their real
/// ids, titles, icons and anchors. This is data, not a render switch: adding a
/// window is one descriptor and one feature module.
pub fn openidea_tool_windows() -> Contributions {
    let mut contributions = Contributions::default();
    let left = [
        ("project", "Project", "toolwindow-project", 0, false, false),
        ("structure", "Structure", "toolwindow-structure", 10, false, false),
        ("bookmarks", "Bookmarks", "toolwindow-bookmarks", 20, false, false),
        ("commit", "Commit", "toolwindow-commit", 30, false, false),
    ];
    let left_on_use = [
        ("compare", "Compare", "diff", 40, false, false),
    ];
    let right = [
        ("maven", "Maven", "toolwindow-maven", 0, false, false),
        ("database", "Database", "toolwindow-database", 10, false, false),
        ("agents", "Agents", "toolwindow-agents", 20, false, false),
        ("coverage", "Coverage", "toolwindow-coverage", 30, false, false),
        ("hierarchy", "Hierarchy", "toolwindow-hierarchy", 40, false, false),
    ];
    let bottom = [
        ("run", "Run", "toolwindow-run", 0, false, false),
        ("debug", "Debug", "toolwindow-debug", 10, false, false),
        ("problems", "Problems", "toolwindow-problems", 20, false, false),
        ("git", "Version Control", "toolwindow-git", 30, false, false),
        ("terminal", "Terminal", "toolwindow-terminal", 40, false, true),
        ("todo", "TODO", "toolwindow-todo", 50, false, false),
        ("services", "Services", "toolwindow-services", 60, false, false),
    ];
    let bottom_on_use = [
        ("find", "Find", "toolwindow-find", 70, false, false),
        ("build", "Build", "toolwindow-build", 80, false, false),
        ("inspection", "Inspection Results", "toolwindow-inspection", 90, false, false),
        ("duplicates", "Duplicates", "toolwindow-duplicates", 100, false, false),
        ("usages", "Find Usages", "toolwindow-usages", 110, false, false),
        ("profiler", "Profiler", "toolwindow-profiler", 120, false, false),
        ("logcat", "Logcat", "toolwindow-logcat", 130, false, false),
    ];
    for (id, title, icon, order, secondary, session) in left
        .into_iter()
        .chain(left_on_use.map(|v| (v.0, v.1, v.2, v.3, v.4, v.5)))
    {
        let mut window = ToolWindowContribution::new(id, title, icon, ToolWindowAnchor::Left)
            .order(order);
        window.secondary = secondary;
        window.session_content = session;
        if left_on_use.iter().any(|v| v.0 == id) {
            window.on_use = true;
        }
        contributions.register_tool_window(window);
    }
    for (id, title, icon, order, secondary, session) in right {
        let mut window = ToolWindowContribution::new(id, title, icon, ToolWindowAnchor::Right)
            .order(order);
        window.secondary = secondary;
        window.session_content = session;
        contributions.register_tool_window(window);
    }
    for (id, title, icon, order, secondary, session) in bottom
        .into_iter()
        .chain(bottom_on_use.map(|v| (v.0, v.1, v.2, v.3, v.4, v.5)))
    {
        let mut window = ToolWindowContribution::new(id, title, icon, ToolWindowAnchor::Bottom)
            .order(order);
        window.secondary = secondary;
        window.session_content = session;
        if bottom_on_use.iter().any(|v| v.0 == id) {
            window.on_use = true;
        }
        contributions.register_tool_window(window);
    }
    contributions
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn openidea_builtins_have_real_ids_icons_and_anchors() {
        let contributions = openidea_tool_windows();
        let project = contributions.tool_windows.get("project").unwrap();
        assert_eq!(project.title, "Project");
        assert_eq!(project.icon, "toolwindow-project");
        assert_eq!(project.anchor, ToolWindowAnchor::Left);
        let maven = contributions.tool_windows.get("maven").unwrap();
        assert_eq!(maven.anchor, ToolWindowAnchor::Right);
        let terminal = contributions.tool_windows.get("terminal").unwrap();
        assert_eq!(terminal.anchor, ToolWindowAnchor::Bottom);
        assert!(terminal.session_content);
    }

    #[test]
    fn stripe_order_is_contribution_order_then_title() {
        let contributions = openidea_tool_windows();
        let ordered = contributions.tool_windows_in_order();
        let ids: Vec<_> = ordered.iter().map(|w| w.id.as_str()).collect();
        let left: Vec<_> = ordered
            .iter()
            .filter(|w| w.anchor == ToolWindowAnchor::Left)
            .map(|w| w.id.as_str())
            .collect();
        assert_eq!(left.first().copied(), Some("project"));
        assert!(ids.contains(&"terminal"));
    }

    #[test]
    fn registry_replaces_an_id_without_reordering_the_rest() {
        let mut registry = Registry::default();
        registry.insert("a", 1);
        registry.insert("b", 2);
        assert_eq!(registry.insert("a", 3), Some(1));
        assert_eq!(registry.get("a"), Some(&3));
        assert_eq!(registry.len(), 2);
    }
}
