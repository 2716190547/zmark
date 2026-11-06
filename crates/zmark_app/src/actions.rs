use gpui::actions;

actions!(
    zmark,
    [
        ToggleSourceMode,
        ToggleSplitMode,
        TogglePreviewMode,
        ToggleDrawer,
        ToggleSettings,
        OpenFileDialog,
        SaveFile,
        ExportHtml
    ]
);
