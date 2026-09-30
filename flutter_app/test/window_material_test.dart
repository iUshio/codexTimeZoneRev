import 'dart:io';

import 'package:codex_timezone/app/application.dart';
import 'package:codex_timezone/services/window_material.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:forui/forui.dart';
import 'package:timezone/data/latest.dart' as timezone;

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  const channel = MethodChannel('com.alexmercerind/flutter_acrylic');
  final calls = <MethodCall>[];
  setUp(() {
    calls.clear();
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(channel, (call) async {
          calls.add(call);
          return null;
        });
  });
  tearDown(
    () => TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(channel, null),
  );

  test('solid background opt-out never invokes the native effect', () async {
    expect(await WindowMaterial.initialize(enabled: false), false);
    expect(calls, isEmpty);
  });

  testWidgets(
    'acrylic surfaces and native appearance follow Forui theme changes',
    (tester) async {
      timezone.initializeTimeZones();
      expect(await WindowMaterial.initialize(enabled: true), true);
      tester.view.physicalSize = const Size(1280, 900);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      await tester.pumpWidget(
        const LauncherApplication(
          preview: true,
          acrylicEnabled: true,
          initialAppearance: 'light',
        ),
      );
      await tester.pumpAndSettle();
      final lightTheme = tester.element(find.text('Codex 时区启动器')).theme;
      final light = lightTheme.colors;
      expect(
        tester
            .element(find.text('Codex 时区启动器'))
            .theme
            .scaffoldStyle
            .backgroundColor
            .a,
        0,
      );
      expect(light.background.a, 1);
      expect(light.card.a, 1);
      expect(
        (lightTheme.cardStyle.decoration as ShapeDecoration).color!.a,
        closeTo(0.54, 0.001),
      );
      expect(light.foreground.a, 1);
      await tester.tap(find.text('浅色').first);
      await tester.pumpAndSettle();
      await tester.tap(find.text('深色').last);
      await tester.pumpAndSettle();
      final effect = calls.lastWhere((call) => call.method == 'SetEffect');
      expect(effect.arguments['dark'], true);
      expect(effect.arguments['effect'], 4);
      final darkTheme = tester.element(find.text('Codex 时区启动器')).theme;
      expect(darkTheme.colors.card.a, 1);
      expect(
        (darkTheme.cardStyle.decoration as ShapeDecoration).color!.a,
        closeTo(0.48, 0.001),
      );
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
    },
    skip: !Platform.isWindows,
  );
}
