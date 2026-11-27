//! OpenIDEA workbench semantics shared by the native shell and tests.

pub mod layout;
pub mod registry;

pub use layout::{
    AnchorSizes, AnchorSplits, LayoutSnapshot, LayoutState, SlotRecord, ToolWindowAnchor,
    ToolWindowPlacement, ToolWindowSlot, ToolWindowViewMode, slot_of,
};
pub use registry::{
    ActionContribution, Align, Contributions, EditorKindContribution, MenuGroupContribution,
    OverlayContribution, Registry, SettingsPageContribution, StatusItemContribution,
    TitleItemContribution, ToolWindowContribution, openidea_tool_windows,
};
