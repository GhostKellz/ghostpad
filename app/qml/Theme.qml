// GhostPad color palettes.
//
// Provides explicit palettes for the custom themes (Light, Dark, and the three
// Tokyo Night variants). The "system" theme is handled by Kirigami inheritance
// and intentionally has no entry here.
//
// Each palette maps to the writable Kirigami.Theme color roles so that
// applyTheme() in Main.qml can assign them and have the colors cascade to
// child items. Tokyo Night hex values come from the upstream tokyonight.nvim
// palette.
import QtQuick 2.15

QtObject {
    readonly property var palettes: ({
        "light": {
            background: "#fafafa",
            text: "#1a1a1a",
            disabledText: "#9e9e9e",
            alternateBackground: "#f0f0f0",
            highlight: "#3584e4",
            negativeText: "#c0392b",
            neutralText: "#b8860b",
            positiveText: "#27ae60"
        },
        "dark": {
            background: "#1e1e1e",
            text: "#e6e6e6",
            disabledText: "#8a8a8a",
            alternateBackground: "#2a2a2a",
            highlight: "#4a90d9",
            negativeText: "#e06c75",
            neutralText: "#e0af68",
            positiveText: "#98c379"
        },
        "tokyo_night": {
            background: "#1a1b26",
            text: "#c0caf5",
            disabledText: "#565f89",
            alternateBackground: "#292e42",
            highlight: "#7aa2f7",
            negativeText: "#f7768e",
            neutralText: "#e0af68",
            positiveText: "#9ece6a"
        },
        "tokyo_night_storm": {
            background: "#24283b",
            text: "#c0caf5",
            disabledText: "#565f89",
            alternateBackground: "#292e42",
            highlight: "#7aa2f7",
            negativeText: "#f7768e",
            neutralText: "#e0af68",
            positiveText: "#9ece6a"
        },
        "tokyo_night_moon": {
            background: "#222436",
            text: "#c8d3f5",
            disabledText: "#636da6",
            alternateBackground: "#2f334d",
            highlight: "#82aaff",
            negativeText: "#ff757f",
            neutralText: "#ffc777",
            positiveText: "#c3e88d"
        }
    })

    // Ordered list of selectable themes for the settings combo box.
    readonly property var options: [
        { value: "system", label: qsTr("System") },
        { value: "light", label: qsTr("Light") },
        { value: "dark", label: qsTr("Dark") },
        { value: "tokyo_night", label: qsTr("Tokyo Night") },
        { value: "tokyo_night_storm", label: qsTr("Tokyo Night Storm") },
        { value: "tokyo_night_moon", label: qsTr("Tokyo Night Moon") }
    ]

    // Returns the palette for a custom theme, or null for "system"/unknown.
    function colorsFor(variant) {
        return palettes[variant] || null;
    }

    // Returns the index of a theme value within `options` (0/"system" fallback).
    function indexOf(variant) {
        for (var i = 0; i < options.length; ++i) {
            if (options[i].value === variant)
                return i;
        }
        return 0;
    }
}
