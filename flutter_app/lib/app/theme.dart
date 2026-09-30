import 'package:forui/forui.dart';

// Build dependent component styles from a single desktop type scale, including
// explicit CJK fallbacks instead of relying on platform font discovery alone.
FThemeData launcherTheme(bool dark, {bool acrylic = false}) {
  final base =
      (dark ? FTheme.neutral.dark.desktop : FTheme.neutral.light.desktop)
          .colors;
  // Forui also uses card for select popovers and text fields. Keep this shared
  // token opaque and apply the acrylic tint only to the page's card surfaces.
  final colors = base;
  final typeface = FTypeface.inherit(
    colors: colors,
    touch: false,
    fontFamilyFallback: const [
      'Microsoft YaHei',
      'PingFang SC',
      'Noto Sans CJK SC',
    ],
  );
  final theme = FThemeData(
    colors: colors,
    touch: false,
    typography: FTypography(display: typeface, body: typeface),
  );
  // The page backdrop and cards reveal the native material; menus and enabled
  // input surfaces retain their opaque fill above the page content.
  return acrylic
      ? theme.copyWith(
          scaffoldStyle: theme.scaffoldStyle.copyWith(
            backgroundColor: base.background.withValues(alpha: 0),
          ),
          cardStyle: theme.cardStyle.copyWith(
            decoration: DecorationDelta.shapeDelta(
              color: base.card.withValues(alpha: dark ? 0.48 : 0.54),
            ),
          ),
        )
      : theme;
}

final lightLauncherTheme = launcherTheme(false);
final darkLauncherTheme = launcherTheme(true);
final lightAcrylicTheme = launcherTheme(false, acrylic: true);
final darkAcrylicTheme = launcherTheme(true, acrylic: true);
