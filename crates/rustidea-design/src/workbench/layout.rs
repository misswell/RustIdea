//! The OpenIDEA tool-window layout state machine.
//!
//! Each side has a primary and a split area; showing another window in the
//! same area replaces it. Placements, sizes, split ratios and View Modes are
//! persisted per project root. This is a direct port of
//! `src/workbench/toolwindows/store.ts`, kept renderer-independent so it can be
//! tested without a window.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::registry::{Contributions, ToolWindowContribution};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ToolWindowAnchor {
    Left,
    Bottom,
    Right,
}

impl ToolWindowAnchor {
    pub const ALL: [Self; 3] = [Self::Left, Self::Bottom, Self::Right];

    pub fn slots(self) -> [ToolWindowSlot; 2] {
        match self {
            Self::Left => [ToolWindowSlot::Left, ToolWindowSlot::LeftSplit],
            Self::Bottom => [ToolWindowSlot::Bottom, ToolWindowSlot::BottomSplit],
            Self::Right => [ToolWindowSlot::Right, ToolWindowSlot::RightSplit],
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Bottom => "bottom",
            Self::Right => "right",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ToolWindowSlot {
    Left,
    LeftSplit,
    Bottom,
    BottomSplit,
    Right,
    RightSplit,
}

impl ToolWindowSlot {
    pub const ALL: [Self; 6] = [
        Self::Left,
        Self::LeftSplit,
        Self::Bottom,
        Self::BottomSplit,
        Self::Right,
        Self::RightSplit,
    ];

    pub fn anchor(self) -> ToolWindowAnchor {
        match self {
            Self::Left | Self::LeftSplit => ToolWindowAnchor::Left,
            Self::Bottom | Self::BottomSplit => ToolWindowAnchor::Bottom,
            Self::Right | Self::RightSplit => ToolWindowAnchor::Right,
        }
    }

    pub fn is_split(self) -> bool {
        matches!(self, Self::LeftSplit | Self::BottomSplit | Self::RightSplit)
    }

    pub fn partner(self) -> Self {
        match self {
            Self::Left => Self::LeftSplit,
            Self::LeftSplit => Self::Left,
            Self::Bottom => Self::BottomSplit,
            Self::BottomSplit => Self::Bottom,
            Self::Right => Self::RightSplit,
            Self::RightSplit => Self::Right,
        }
    }
}

pub fn slot_of(contribution: &ToolWindowContribution) -> ToolWindowSlot {
    match (contribution.anchor, contribution.secondary) {
        (ToolWindowAnchor::Left, false) => ToolWindowSlot::Left,
        (ToolWindowAnchor::Left, true) => ToolWindowSlot::LeftSplit,
        (ToolWindowAnchor::Bottom, false) => ToolWindowSlot::Bottom,
        (ToolWindowAnchor::Bottom, true) => ToolWindowSlot::BottomSplit,
        (ToolWindowAnchor::Right, false) => ToolWindowSlot::Right,
        (ToolWindowAnchor::Right, true) => ToolWindowSlot::RightSplit,
    }
}

/// Visible window id per area. OpenIDEA's persisted keys are exactly these.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SlotRecord {
    pub left: Option<String>,
    #[serde(rename = "left-split")]
    pub left_split: Option<String>,
    pub bottom: Option<String>,
    #[serde(rename = "bottom-split")]
    pub bottom_split: Option<String>,
    pub right: Option<String>,
    #[serde(rename = "right-split")]
    pub right_split: Option<String>,
}

impl Default for SlotRecord {
    fn default() -> Self {
        Self {
            left: Some("project".into()),
            left_split: None,
            bottom: None,
            bottom_split: None,
            right: None,
            right_split: None,
        }
    }
}

impl SlotRecord {
    pub fn empty() -> Self {
        Self {
            left: None,
            left_split: None,
            bottom: None,
            bottom_split: None,
            right: None,
            right_split: None,
        }
    }

    pub fn get(&self, slot: ToolWindowSlot) -> Option<&str> {
        match slot {
            ToolWindowSlot::Left => self.left.as_deref(),
            ToolWindowSlot::LeftSplit => self.left_split.as_deref(),
            ToolWindowSlot::Bottom => self.bottom.as_deref(),
            ToolWindowSlot::BottomSplit => self.bottom_split.as_deref(),
            ToolWindowSlot::Right => self.right.as_deref(),
            ToolWindowSlot::RightSplit => self.right_split.as_deref(),
        }
    }

    pub fn set(&mut self, slot: ToolWindowSlot, id: Option<String>) {
        match slot {
            ToolWindowSlot::Left => self.left = id,
            ToolWindowSlot::LeftSplit => self.left_split = id,
            ToolWindowSlot::Bottom => self.bottom = id,
            ToolWindowSlot::BottomSplit => self.bottom_split = id,
            ToolWindowSlot::Right => self.right = id,
            ToolWindowSlot::RightSplit => self.right_split = id,
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (ToolWindowSlot, Option<&str>)> {
        [
            (ToolWindowSlot::Left, self.left.as_deref()),
            (ToolWindowSlot::LeftSplit, self.left_split.as_deref()),
            (ToolWindowSlot::Bottom, self.bottom.as_deref()),
            (ToolWindowSlot::BottomSplit, self.bottom_split.as_deref()),
            (ToolWindowSlot::Right, self.right.as_deref()),
            (ToolWindowSlot::RightSplit, self.right_split.as_deref()),
        ]
        .into_iter()
    }

    pub fn ids(&self) -> impl Iterator<Item = &str> {
        self.iter().filter_map(|(_, id)| id)
    }

    pub fn any_visible(&self) -> bool {
        self.ids().next().is_some()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AnchorSizes {
    pub left: u32,
    pub right: u32,
    pub bottom: u32,
}

impl Default for AnchorSizes {
    fn default() -> Self {
        Self {
            left: 240,
            right: 300,
            bottom: 280,
        }
    }
}

impl AnchorSizes {
    pub fn get(&self, anchor: ToolWindowAnchor) -> u32 {
        match anchor {
            ToolWindowAnchor::Left => self.left,
            ToolWindowAnchor::Right => self.right,
            ToolWindowAnchor::Bottom => self.bottom,
        }
    }

    pub fn set(&mut self, anchor: ToolWindowAnchor, value: u32) {
        let value = value.clamp(80, 4000);
        match anchor {
            ToolWindowAnchor::Left => self.left = value,
            ToolWindowAnchor::Right => self.right = value,
            ToolWindowAnchor::Bottom => self.bottom = value,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AnchorSplits {
    pub left: f32,
    pub right: f32,
    pub bottom: f32,
}

impl Default for AnchorSplits {
    fn default() -> Self {
        Self {
            left: 0.5,
            right: 0.5,
            bottom: 0.5,
        }
    }
}

impl AnchorSplits {
    pub fn get(&self, anchor: ToolWindowAnchor) -> f32 {
        match anchor {
            ToolWindowAnchor::Left => self.left,
            ToolWindowAnchor::Right => self.right,
            ToolWindowAnchor::Bottom => self.bottom,
        }
    }

    pub fn set(&mut self, anchor: ToolWindowAnchor, value: f32) {
        let value = value.clamp(MIN_SPLIT, MAX_SPLIT);
        match anchor {
            ToolWindowAnchor::Left => self.left = value,
            ToolWindowAnchor::Right => self.right = value,
            ToolWindowAnchor::Bottom => self.bottom = value,
        }
    }
}

pub const MIN_SPLIT: f32 = 0.1;
pub const MAX_SPLIT: f32 = 0.9;
pub const LAYOUT_VERSION: u32 = 2;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ToolWindowViewMode {
    #[default]
    DockPinned,
    DockUnpinned,
    Undock,
}

/// A user placement override, created by dragging a stripe button or Move To.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolWindowPlacement {
    pub anchor: ToolWindowAnchor,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub split: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order: Option<i32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LayoutSnapshot {
    pub v: u32,
    pub visible: SlotRecord,
    pub sizes: AnchorSizes,
    pub splits: AnchorSplits,
    pub placements: BTreeMap<String, ToolWindowPlacement>,
    pub stripe_buttons: BTreeMap<String, bool>,
    pub view_modes: BTreeMap<String, ToolWindowViewMode>,
}

impl Default for LayoutSnapshot {
    fn default() -> Self {
        Self {
            v: LAYOUT_VERSION,
            visible: SlotRecord::default(),
            sizes: AnchorSizes::default(),
            splits: AnchorSplits::default(),
            placements: BTreeMap::new(),
            stripe_buttons: BTreeMap::new(),
            view_modes: BTreeMap::new(),
        }
    }
}

impl LayoutSnapshot {
    /// Sanitizes data read from disk, following the OpenIDEA rules: sizes are
    /// rounded and clamped, splits outside 0.1–0.9 are ignored, unknown slots
    /// are dropped, and version-1 right-side `secondary` placements keep working.
    pub fn sanitized(mut self) -> Self {
        self.v = LAYOUT_VERSION;
        let defaults = AnchorSizes::default();
        for anchor in ToolWindowAnchor::ALL {
            let size = self.sizes.get(anchor);
            if !(80..=4000).contains(&size) {
                self.sizes.set(anchor, defaults.get(anchor));
            }
        }
        let splits = AnchorSplits::default();
        for anchor in ToolWindowAnchor::ALL {
            if !(MIN_SPLIT..=MAX_SPLIT).contains(&self.splits.get(anchor)) {
                self.splits.set(anchor, splits.get(anchor));
            }
        }
        self
    }
}

#[derive(Clone, Debug)]
pub struct LayoutState {
    pub scope: String,
    pub visible: SlotRecord,
    pub sizes: AnchorSizes,
    pub splits: AnchorSplits,
    /// Tool window containing DOM focus, if any.
    pub focused: Option<String>,
    /// Most recently activated window (target of Jump to Last Tool Window).
    pub last_active: Option<String>,
    /// Windows mounted at least once; hidden windows keep their entity.
    pub mounted: BTreeSet<String>,
    pub maximized: Option<ToolWindowAnchor>,
    pub stashed: Option<SlotRecord>,
    pub stashed_maximized: Option<ToolWindowAnchor>,
    pub placements: BTreeMap<String, ToolWindowPlacement>,
    pub stripe_buttons: BTreeMap<String, bool>,
    pub view_modes: BTreeMap<String, ToolWindowViewMode>,
    pub distraction_free_stash: Option<SlotRecord>,
    pub narrow_stash: Option<BTreeMap<ToolWindowSlot, String>>,
}

impl Default for LayoutState {
    fn default() -> Self {
        let visible = SlotRecord::default();
        Self {
            scope: String::new(),
            mounted: visible.ids().map(str::to_owned).collect(),
            visible,
            sizes: AnchorSizes::default(),
            splits: AnchorSplits::default(),
            focused: None,
            last_active: None,
            maximized: None,
            stashed: None,
            stashed_maximized: None,
            placements: BTreeMap::new(),
            stripe_buttons: BTreeMap::new(),
            view_modes: BTreeMap::new(),
            distraction_free_stash: None,
            narrow_stash: None,
        }
    }
}

impl LayoutState {
    pub fn load_with(registry: &Contributions, scope: impl Into<String>, snapshot: Option<LayoutSnapshot>) -> Self {
        let scope = scope.into();
        let Some(snapshot) = snapshot else {
            return Self {
                scope,
                ..Self::default()
            };
        };
        let snapshot = snapshot.sanitized();
        let mut state = Self {
            scope,
            visible: snapshot.visible,
            sizes: snapshot.sizes,
            splits: snapshot.splits,
            focused: None,
            last_active: None,
            mounted: BTreeSet::new(),
            maximized: None,
            stashed: None,
            stashed_maximized: None,
            placements: snapshot.placements,
            stripe_buttons: snapshot.stripe_buttons,
            view_modes: snapshot.view_modes,
            distraction_free_stash: None,
            narrow_stash: None,
        };
        state.drop_session_windows(registry);
        state.reconcile(registry);
        state.mounted = state.visible.ids().map(str::to_owned).collect();
        state
    }

    pub fn snapshot(&self) -> LayoutSnapshot {
        LayoutSnapshot {
            v: LAYOUT_VERSION,
            visible: self.visible.clone(),
            sizes: self.sizes,
            splits: self.splits,
            placements: self.placements.clone(),
            stripe_buttons: self.stripe_buttons.clone(),
            view_modes: self.view_modes.clone(),
        }
    }

    /// Persistable snapshot: windows whose content doesn't outlive the project
    /// (Run, Debug, terminals) are not reopened with it, as in OpenIDEA.
    pub fn snapshot_filtered(&self, registry: &Contributions) -> LayoutSnapshot {
        let mut snapshot = self.snapshot();
        for slot in ToolWindowSlot::ALL {
            let Some(id) = snapshot.visible.get(slot).map(str::to_owned) else {
                continue;
            };
            if registry
                .tool_windows
                .get(&id)
                .is_some_and(|window| window.session_content)
            {
                snapshot.visible.set(slot, None);
            }
        }
        snapshot
    }

    fn drop_session_windows(&mut self, registry: &Contributions) {
        for slot in ToolWindowSlot::ALL {
            let Some(id) = self.visible.get(slot).map(str::to_owned) else {
                continue;
            };
            if registry
                .tool_windows
                .get(&id)
                .is_some_and(|window| window.session_content)
            {
                self.visible.set(slot, None);
            }
        }
    }

    pub fn placed<'a>(&'a self, contribution: &'a ToolWindowContribution) -> ToolWindowContribution {
        let Some(placement) = self.placements.get(&contribution.id) else {
            return contribution.clone();
        };
        let mut placed = contribution.clone();
        placed.anchor = placement.anchor;
        placed.secondary = placement
            .split
            .unwrap_or(contribution.anchor == placement.anchor && contribution.secondary);
        placed.order = placement.order.or(contribution.order);
        placed
    }

    pub fn slot_of(&self, registry: &Contributions, id: &str) -> Option<ToolWindowSlot> {
        registry
            .tool_windows
            .get(id)
            .map(|window| slot_of(&self.placed(window)))
    }

    pub fn anchor_of(&self, registry: &Contributions, id: &str) -> Option<ToolWindowAnchor> {
        self.slot_of(registry, id).map(ToolWindowSlot::anchor)
    }

    pub fn effective_maximized(&self, registry: &Contributions) -> Option<ToolWindowAnchor> {
        let maximized = self.maximized?;
        let visible_here = maximized.slots().into_iter().any(|slot| {
            self.visible
                .get(slot)
                .and_then(|id| registry.tool_windows.get(id))
                .is_some_and(|window| slot_of(&self.placed(window)) == slot)
        });
        visible_here.then_some(maximized)
    }

    pub fn is_visible(&self, registry: &Contributions, id: &str) -> bool {
        let Some(slot) = self.slot_of(registry, id) else {
            return false;
        };
        let effective_max = self.effective_maximized(registry);
        (effective_max.is_none() || effective_max == Some(slot.anchor()))
            && self.visible.get(slot) == Some(id)
    }

    pub fn show(&mut self, registry: &Contributions, id: &str, focus: bool) {
        let Some(slot) = self.slot_of(registry, id) else {
            return;
        };
        let anchor = slot.anchor();
        self.last_active = Some(id.to_owned());
        self.mounted.insert(id.to_owned());
        self.visible.set(slot, Some(id.to_owned()));
        self.stripe_buttons.entry(id.to_owned()).or_insert(true);
        self.stashed = None;
        if self.maximized.is_some_and(|maximized| maximized != anchor) {
            self.maximized = None;
        }
        if focus {
            self.focused = Some(id.to_owned());
        }
        self.keep_narrow_stash_to_hidden_sides();
    }

    pub fn hide(&mut self, registry: &Contributions, id: &str) {
        let Some(slot) = self.slot_of(registry, id) else {
            return;
        };
        if self.visible.get(slot) != Some(id) {
            return;
        }
        self.visible.set(slot, None);
        let side_empty = !self
            .visible
            .iter()
            .any(|(candidate, value)| candidate.anchor() == slot.anchor() && value.is_some());
        if self.maximized == Some(slot.anchor()) && side_empty {
            self.maximized = None;
        }
        if self.last_active.as_deref() == Some(id) {
            self.last_active = None;
        }
        if self.focused.as_deref() == Some(id) {
            self.focused = self
                .visible
                .get(slot.partner())
                .map(str::to_owned)
                .filter(|_| self.maximized == Some(slot.anchor()));
        }
        self.keep_narrow_stash_to_hidden_sides();
    }

    pub fn toggle(&mut self, registry: &Contributions, id: &str) {
        if self.is_visible(registry, id) {
            self.hide(registry, id);
        } else {
            self.show(registry, id, true);
        }
    }

    /// The `toolwindow.<id>` action: hidden → show + focus; visible but
    /// unfocused → focus; visible and focused → hide (IDEA behaviour).
    pub fn activate(&mut self, registry: &Contributions, id: &str) {
        if !self.is_visible(registry, id) {
            self.show(registry, id, true);
        } else if self.focused.as_deref() != Some(id) {
            self.focused = Some(id.to_owned());
            self.last_active = Some(id.to_owned());
        } else {
            self.hide(registry, id);
        }
    }

    pub fn focus(&mut self, registry: &Contributions, id: &str) {
        if !self.is_visible(registry, id) {
            self.show(registry, id, true);
        } else {
            self.focused = Some(id.to_owned());
            self.last_active = Some(id.to_owned());
        }
    }

    pub fn jump_to_last(&mut self, registry: &Contributions) {
        let id = self
            .last_active
            .clone()
            .filter(|id| registry.tool_windows.contains(id))
            .or_else(|| registry.tool_windows.contains("project").then(|| "project".to_owned()));
        if let Some(id) = id {
            self.focus(registry, &id);
        }
    }

    pub fn toggle_maximize(&mut self, anchor: ToolWindowAnchor) {
        if self.maximized == Some(anchor) {
            self.maximized = None;
        } else if self
            .visible
            .iter()
            .any(|(slot, id)| slot.anchor() == anchor && id.is_some())
        {
            self.maximized = Some(anchor);
        }
    }

    pub fn hide_all(&mut self) {
        if self.visible.any_visible() {
            self.stashed = Some(self.visible.clone());
            self.stashed_maximized = self.maximized;
            self.visible = SlotRecord::empty();
            self.maximized = None;
            self.focused = None;
        } else if let Some(stashed) = self.stashed.take() {
            self.visible = stashed;
            self.maximized = self.stashed_maximized.take();
            self.mounted.extend(self.visible.ids().map(str::to_owned));
        }
    }

    pub fn move_to(&mut self, registry: &Contributions, id: &str, target: ToolWindowSlot, before: Option<&str>) {
        if !registry.tool_windows.contains(id) {
            return;
        }
        let Some(old_slot) = self.slot_of(registry, id) else {
            return;
        };
        let was_visible = self.visible.get(old_slot) == Some(id);
        let mut members: Vec<String> = registry
            .tool_windows
            .values()
            .map(|window| self.placed(window))
            .filter(|window| window.id != id && slot_of(window) == target)
            .map(|window| window.id)
            .collect();
        members.sort_by(|a, b| {
            let a_order = registry
                .tool_windows
                .get(a)
                .map(|w| self.placed(w).order.unwrap_or(100))
                .unwrap_or(100);
            let b_order = registry
                .tool_windows
                .get(b)
                .map(|w| self.placed(w).order.unwrap_or(100))
                .unwrap_or(100);
            a_order.cmp(&b_order).then_with(|| a.cmp(b))
        });
        let insert_at = before
            .and_then(|before| members.iter().position(|member| member == before))
            .unwrap_or(members.len());
        members.insert(insert_at, id.to_owned());
        for (index, member) in members.iter().enumerate() {
            let order = (index as i32 + 1) * 10;
            if member == id {
                self.placements.insert(
                    member.clone(),
                    ToolWindowPlacement {
                        anchor: target.anchor(),
                        split: Some(target.is_split()),
                        order: Some(order),
                    },
                );
            } else if let Some(placement) = self.placements.get_mut(member) {
                placement.order = Some(order);
            } else if let Some(window) = registry.tool_windows.get(member) {
                self.placements.insert(
                    member.clone(),
                    ToolWindowPlacement {
                        anchor: window.anchor,
                        split: Some(slot_of(window).is_split()),
                        order: Some(order),
                    },
                );
            }
        }
        if was_visible && old_slot != target {
            self.visible.set(old_slot, None);
            self.visible.set(target, Some(id.to_owned()));
        }
        if self.maximized == Some(old_slot.anchor()) && old_slot.anchor() != target.anchor() {
            self.maximized = None;
        }
    }

    pub fn reset_placements(&mut self, registry: &Contributions) {
        for slot in ToolWindowSlot::ALL {
            let Some(id) = self.visible.get(slot).map(str::to_owned) else {
                continue;
            };
            if let Some(window) = registry.tool_windows.get(&id) {
                if slot_of(window) != slot {
                    self.visible.set(slot, None);
                }
            }
        }
        self.placements.clear();
        self.stripe_buttons.clear();
        self.view_modes.clear();
    }

    pub fn has_stripe_button(&self, contribution: &ToolWindowContribution) -> bool {
        self.stripe_buttons
            .get(&contribution.id)
            .copied()
            .unwrap_or(!contribution.on_use)
    }

    pub fn remove_from_stripe(&mut self, registry: &Contributions, id: &str) {
        self.hide(registry, id);
        self.stripe_buttons.insert(id.to_owned(), false);
    }

    pub fn view_mode(&self, id: &str) -> ToolWindowViewMode {
        self.view_modes
            .get(id)
            .copied()
            .unwrap_or_default()
    }

    pub fn set_view_mode(&mut self, id: &str, mode: ToolWindowViewMode) {
        if mode == ToolWindowViewMode::DockPinned {
            self.view_modes.remove(id);
        } else {
            self.view_modes.insert(id.to_owned(), mode);
        }
    }

    pub fn set_size(&mut self, anchor: ToolWindowAnchor, size: u32) {
        self.sizes.set(anchor, size);
    }

    pub fn set_split(&mut self, anchor: ToolWindowAnchor, split: f32) {
        self.splits.set(anchor, split);
    }

    pub fn enter_distraction_free(&mut self) {
        if self.scope.is_empty() || self.distraction_free_stash.is_some() {
            return;
        }
        self.distraction_free_stash = Some(self.visible.clone());
        self.visible = SlotRecord::empty();
        self.maximized = None;
        self.stashed = None;
        self.focused = None;
    }

    pub fn exit_distraction_free(&mut self) {
        let Some(visible) = self.distraction_free_stash.take() else {
            return;
        };
        self.mounted.extend(visible.ids().map(str::to_owned));
        self.visible = visible;
        self.maximized = None;
        self.stashed = None;
    }

    pub fn collapse_sides_for_narrow_window(&mut self, side_slots: &[ToolWindowSlot]) {
        if self.scope.is_empty() || self.narrow_stash.is_some() || self.distraction_free_stash.is_some() {
            return;
        }
        let mut stashed = BTreeMap::new();
        for slot in side_slots {
            if let Some(id) = self.visible.get(*slot).map(str::to_owned) {
                stashed.insert(*slot, id);
                self.visible.set(*slot, None);
            }
        }
        if !stashed.is_empty() {
            self.narrow_stash = Some(stashed);
            self.maximized = None;
        }
    }

    pub fn refresh_narrow_stash(&mut self, side_slots: &[ToolWindowSlot]) {
        let Some(stash) = self.narrow_stash.as_ref() else {
            return;
        };
        let kept: BTreeMap<_, _> = stash
            .iter()
            .filter(|(slot, _)| side_slots.contains(slot) && self.visible.get(**slot).is_none())
            .map(|(slot, id)| (*slot, id.clone()))
            .collect();
        self.narrow_stash = (!kept.is_empty()).then_some(kept);
    }

    fn keep_narrow_stash_to_hidden_sides(&mut self) {
        let side_slots = [
            ToolWindowSlot::Left,
            ToolWindowSlot::LeftSplit,
            ToolWindowSlot::Right,
            ToolWindowSlot::RightSplit,
        ];
        self.refresh_narrow_stash(&side_slots);
    }

    /// Keeps the layout in step with registrations: a visible window recorded
    /// in an area it no longer docks in moves to its home area when free; ids
    /// that disappeared are removed. Returns whether anything changed.
    pub fn reconcile(&mut self, registry: &Contributions) -> bool {
        let mut changed = false;
        let snapshot = self.visible.clone();
        for (slot, value) in snapshot.iter() {
            let Some(id) = value else {
                continue;
            };
            let Some(window) = registry.tool_windows.get(id) else {
                self.visible.set(slot, None);
                changed = true;
                continue;
            };
            let home = slot_of(&self.placed(window));
            if home != slot {
                self.visible.set(slot, None);
                if self.visible.get(home).is_none() {
                    self.visible.set(home, Some(id.to_owned()));
                }
                changed = true;
            }
        }
        if self
            .effective_maximized(registry)
            .is_none()
            && self.maximized.is_some()
        {
            self.maximized = None;
            changed = true;
        }
        if self
            .focused
            .as_deref()
            .is_some_and(|id| !registry.tool_windows.contains(id))
        {
            self.focused = None;
            changed = true;
        }
        if self
            .last_active
            .as_deref()
            .is_some_and(|id| !registry.tool_windows.contains(id))
        {
            self.last_active = None;
            changed = true;
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workbench::registry::openidea_tool_windows;

    #[test]
    fn default_layout_shows_project_in_the_left_primary_area() {
        let layout = LayoutState::default();
        assert_eq!(layout.visible.left.as_deref(), Some("project"));
        assert_eq!(layout.sizes.left, 240);
        assert_eq!(layout.sizes.right, 300);
        assert_eq!(layout.sizes.bottom, 280);
        assert_eq!(layout.splits.left, 0.5);
        assert!(!layout.visible.any_visible() == false);
    }

    #[test]
    fn opening_a_second_window_in_the_same_area_replaces_it() {
        let registry = openidea_tool_windows();
        let mut layout = LayoutState::default();
        layout.show(&registry, "terminal", true);
        assert_eq!(layout.visible.bottom.as_deref(), Some("terminal"));
        layout.show(&registry, "run", true);
        assert_eq!(layout.visible.bottom.as_deref(), Some("run"));
        assert_eq!(layout.visible.left.as_deref(), Some("project"));
        assert!(layout.is_visible(&registry, "run"));
        assert!(!layout.is_visible(&registry, "terminal"));
    }

    #[test]
    fn split_areas_keep_both_windows_on_a_side() {
        let mut registry = openidea_tool_windows();
        let mut structure = registry.tool_windows.get("structure").unwrap().clone();
        structure.secondary = true;
        registry.register_tool_window(structure);
        let mut layout = LayoutState::default();
        layout.show(&registry, "project", true);
        layout.show(&registry, "structure", true);
        assert_eq!(layout.visible.left.as_deref(), Some("project"));
        assert_eq!(layout.visible.left_split.as_deref(), Some("structure"));
        layout.hide(&registry, "project");
        assert_eq!(layout.visible.left_split.as_deref(), Some("structure"));
    }

    #[test]
    fn maximize_only_applies_to_a_side_with_a_visible_window() {
        let registry = openidea_tool_windows();
        let mut layout = LayoutState::default();
        layout.toggle_maximize(ToolWindowAnchor::Bottom);
        assert_eq!(layout.maximized, None);
        layout.show(&registry, "run", true);
        layout.toggle_maximize(ToolWindowAnchor::Bottom);
        assert_eq!(layout.maximized, Some(ToolWindowAnchor::Bottom));
        layout.hide(&registry, "run");
        assert_eq!(layout.maximized, None);
    }

    #[test]
    fn hide_all_stashes_and_restores_the_layout() {
        let registry = openidea_tool_windows();
        let mut layout = LayoutState::default();
        layout.show(&registry, "run", true);
        layout.hide_all();
        assert!(!layout.visible.any_visible());
        assert!(layout.stashed.is_some());
        layout.hide_all();
        assert_eq!(layout.visible.left.as_deref(), Some("project"));
        assert_eq!(layout.visible.bottom.as_deref(), Some("run"));
    }

    #[test]
    fn move_to_moves_a_visible_window_and_renumbers_the_group() {
        let registry = openidea_tool_windows();
        let mut layout = LayoutState::default();
        layout.show(&registry, "todo", true);
        layout.move_to(&registry, "todo", ToolWindowSlot::LeftSplit, None);
        assert_eq!(layout.visible.bottom, None);
        assert_eq!(layout.visible.left_split.as_deref(), Some("todo"));
        assert_eq!(layout.slot_of(&registry, "todo"), Some(ToolWindowSlot::LeftSplit));

        layout.move_to(&registry, "project", ToolWindowSlot::LeftSplit, Some("todo"));
        assert_eq!(layout.slot_of(&registry, "project"), Some(ToolWindowSlot::LeftSplit));
    }

    #[test]
    fn snapshots_drop_session_windows_but_keep_layout_choices() {
        let registry = openidea_tool_windows();
        let mut layout = LayoutState::default();
        layout.show(&registry, "terminal", true);
        layout.set_size(ToolWindowAnchor::Bottom, 420);
        let snapshot = layout.snapshot_filtered(&registry);
        assert_eq!(snapshot.visible.bottom, None);
        assert_eq!(snapshot.visible.left.as_deref(), Some("project"));
        assert_eq!(snapshot.sizes.bottom, 420);
    }

    #[test]
    fn loading_sanitizes_and_reconciles_a_snapshot() {
        let registry = openidea_tool_windows();
        let snapshot = LayoutSnapshot {
            v: 1,
            visible: SlotRecord {
                left: None,
                left_split: None,
                bottom: Some("maven".into()),
                bottom_split: None,
                right: None,
                right_split: None,
            },
            sizes: AnchorSizes {
                left: 10,
                right: 10_000,
                bottom: 300,
            },
            splits: AnchorSplits {
                left: 0.05,
                right: 0.95,
                bottom: 0.5,
            },
            ..LayoutSnapshot::default()
        };
        let layout = LayoutState::load_with(&registry, "/project", Some(snapshot));
        assert_eq!(layout.visible.bottom, None);
        assert_eq!(layout.visible.right.as_deref(), Some("maven"));
        assert_eq!(layout.sizes.left, 240);
        assert_eq!(layout.sizes.right, 300);
        assert_eq!(layout.splits.left, 0.5);
        assert_eq!(layout.splits.right, 0.5);
    }

    #[test]
    fn distraction_free_restores_the_remembered_layout() {
        let registry = openidea_tool_windows();
        let mut layout = LayoutState::default();
        layout.scope = "/project".into();
        layout.show(&registry, "run", true);
        layout.enter_distraction_free();
        assert!(!layout.visible.any_visible());
        layout.exit_distraction_free();
        assert_eq!(layout.visible.left.as_deref(), Some("project"));
        assert_eq!(layout.visible.bottom.as_deref(), Some("run"));
    }

    #[test]
    fn stripe_visibility_follows_the_contribution_until_the_user_decides() {
        let registry = openidea_tool_windows();
        let mut layout = LayoutState::default();
        let build = registry.tool_windows.get("build").unwrap();
        assert!(build.on_use);
        assert!(!layout.has_stripe_button(build));
        layout.remove_from_stripe(&registry, "build");
        assert!(!layout.has_stripe_button(build));
        layout.show(&registry, "build", true);
        // Showing does not undo an explicit "Hide" from the stripe; it stays
        // under the stripe's "…" until the user adds it again.
        assert!(!layout.has_stripe_button(build));
    }
}
