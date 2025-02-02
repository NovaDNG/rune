import 'package:flutter/material.dart';

import '../config/theme.dart';
import '../constants/configurations.dart';

import 'api/get_primary_color.dart';

import 'playing_item.dart';
import 'settings_manager.dart';

class ThemeColorManager {
  static final ThemeColorManager _instance = ThemeColorManager._internal();
  factory ThemeColorManager() => _instance;
  ThemeColorManager._internal();

  bool _isDynamicColorEnabled = false;
  Color? _userSelectedColor;
  PlayingItem? _currentPlayingItem;

  Future<void> initialize() async {
    final settingsManager = SettingsManager();

    // 1. Read dynamic color settings
    final bool? enableDynamicColors =
        await settingsManager.getValue<bool>(kEnableDynamicColorsKey);
    _isDynamicColorEnabled = enableDynamicColors ?? false;

    // 2. Read the user's selected theme color
    final int? themeColor = await settingsManager.getValue<int>(kThemeColorKey);
    if (themeColor != null) {
      _userSelectedColor = Color(themeColor);
    }
  }

  // Update dynamic color settings
  Future<void> updateDynamicColorSetting(bool enabled) async {
    _isDynamicColorEnabled = enabled;

    if (enabled) {
      // If dynamic colors are enabled and a song is currently playing, apply the cover color immediately
      if (_currentPlayingItem != null) {
        await handleCoverArtColorChange(_currentPlayingItem!);
      } else {
        appTheme.updateThemeColor(_userSelectedColor);
      }
    } else {
      // If dynamic colors are disabled, decide which color to use based on user settings
      // If the user chose to follow the system, pass in null
      appTheme.updateThemeColor(_userSelectedColor);
    }
  }

  // Update the user's selected theme color
  void updateUserSelectedColor(Color? color) {
    _userSelectedColor = color;
    // If dynamic colors are not enabled, update the theme based on user selection
    if (!_isDynamicColorEnabled) {
      appTheme.updateThemeColor(color);
    } else {
      if (_currentPlayingItem != null) {
        handleCoverArtColorChange(_currentPlayingItem!);
      } else {
        appTheme.updateThemeColor(color);
      }
    }
  }

  // Handle cover art color change
  Future<void> handleCoverArtColorChange(PlayingItem item) async {
    _currentPlayingItem = item;

    if (!_isDynamicColorEnabled) return;

    final primaryColor = await getPrimaryColor(item);
    if (primaryColor != null) {
      appTheme.updateThemeColor(Color(primaryColor));
    } else {
      appTheme.updateThemeColor(_userSelectedColor);
    }
  }

  // Get the current color that should be used
  Future<void> refreshCurrentColor() async {
    if (_isDynamicColorEnabled && _currentPlayingItem != null) {
      await handleCoverArtColorChange(_currentPlayingItem!);
    } else {
      appTheme.updateThemeColor(_userSelectedColor);
    }
  }
}
